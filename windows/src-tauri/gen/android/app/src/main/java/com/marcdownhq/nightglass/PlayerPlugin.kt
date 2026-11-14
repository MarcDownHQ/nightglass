package com.marcdownhq.nightglass

import android.app.Activity
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.View
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.VideoSize
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.extractor.DefaultExtractorsFactory
import androidx.media3.extractor.ts.DefaultTsPayloadReaderFactory
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray
import org.json.JSONObject

@InvokeArg
class EventsArgs {
    lateinit var channel: Channel
}

@InvokeArg
class PlayArgs {
    lateinit var url: String
    var headers: Map<String, String>? = null
    var options: String? = null
}

@InvokeArg
class CommandArgs {
    lateinit var args: Array<String>
}

@InvokeArg
class SetArgs {
    lateinit var name: String
    lateinit var value: String
}

@InvokeArg
class UrlArgs {
    lateinit var url: String
}

@InvokeArg
class PlaceArgs {
    var x: Int = 0
    var y: Int = 0
    var w: Int = 0
    var h: Int = 0
}

/**
 * The video player of the Android TV build. The page was written for mpv: it sends mpv commands and properties and listens
 * to mpv events. This plugin takes the same ones (`play`, `command`, `set`, `place`) and sends the same events back
 * (`property`, `end-file`, `playback-restart`) from a Media3 player drawing into a surface behind the transparent webview.
 */
@TauriPlugin
class PlayerPlugin(private val activity: Activity) : Plugin(activity) {
    private val main = Handler(Looper.getMainLooper())
    private var events: Channel? = null
    private var webView: WebView? = null
    private var view: PlayerView? = null
    private var player: ExoPlayer? = null

    private var volume = 100
    private var muted = false
    private var textOff = false
    private var lastState = Player.STATE_IDLE
    private var lastPaused: Boolean? = null
    private var lastTime = Double.NEGATIVE_INFINITY
    private var lastDuration = Double.NEGATIVE_INFINITY
    private var lastCache = Double.NEGATIVE_INFINITY

    /** mpv's track ids, 1-based per type, and what they pick in the player. */
    private class Pick(val type: String, val group: Tracks.Group, val index: Int)
    private var picks = HashMap<Pair<String, Int>, Pick>()

    private val ticker = object : Runnable {
        override fun run() {
            tick()
            main.postDelayed(this, 250)
        }
    }

    override fun load(webView: WebView) {
        this.webView = webView
        // The remote's Back is the page's Escape (close the player, leave a menu); Home leaves the app.
        (activity as? ComponentActivity)?.onBackPressedDispatcher?.addCallback(
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    webView.evaluateJavascript(
                        "window.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}))",
                        null,
                    )
                }
            },
        )
        main.post {
            // Transparent page over a surface: where the page paints nothing, the video shows.
            webView.setBackgroundColor(Color.TRANSPARENT)
            val parent = webView.parent as? ViewGroup ?: return@post
            val v = PlayerView(activity).apply {
                useController = false
                resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
                setShutterBackgroundColor(Color.BLACK)
                isFocusable = false
                isFocusableInTouchMode = false
                visibility = View.GONE
            }
            parent.addView(v, 0, FrameLayout.LayoutParams(1, 1))
            view = v
        }
    }

    override fun onDestroy() {
        main.removeCallbacks(ticker)
        player?.release()
        player = null
    }

    private fun send(obj: JSONObject) {
        val e = events ?: return
        e.send(JSObject(obj.toString()))
    }

    private fun property(name: String, value: Any?) {
        send(JSONObject().put("event", "property").put("name", name).put("value", value ?: JSONObject.NULL))
    }

    private fun ensurePlayer(): ExoPlayer {
        player?.let { return it }
        val renderers = DefaultRenderersFactory(activity)
            .setEnableDecoderFallback(true)
            .setExtensionRendererMode(DefaultRenderersFactory.EXTENSION_RENDERER_MODE_OFF)
        val p = ExoPlayer.Builder(activity, renderers)
            .setAudioAttributes(
                AudioAttributes.Builder().setContentType(C.AUDIO_CONTENT_TYPE_MOVIE).setUsage(C.USAGE_MEDIA).build(),
                true,
            )
            .setHandleAudioBecomingNoisy(true)
            .build()
        p.addListener(listener)
        p.playWhenReady = true
        player = p
        view?.player = p
        applyVolume()
        main.postDelayed(ticker, 250)
        return p
    }

    private val listener = object : Player.Listener {
        override fun onPlaybackStateChanged(state: Int) {
            val p = player ?: return
            when (state) {
                Player.STATE_BUFFERING -> property("paused-for-cache", true)
                Player.STATE_READY -> {
                    property("paused-for-cache", false)
                    if (lastState != Player.STATE_READY) send(JSONObject().put("event", "playback-restart"))
                    property("duration", durationSeconds(p))
                }
                Player.STATE_ENDED -> send(JSONObject().put("event", "end-file").put("reason", "eof").put("error", JSONObject.NULL))
            }
            lastState = state
        }

        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) = reportPause()

        override fun onPlayerError(error: PlaybackException) {
            send(
                JSONObject().put("event", "end-file").put("reason", "error")
                    .put("error", "${error.errorCodeName}: ${error.cause?.message ?: error.message ?: ""}"),
            )
        }

        override fun onVideoSizeChanged(size: VideoSize) {
            property("width", size.width)
            property("height", size.height)
        }

        override fun onTracksChanged(tracks: Tracks) = reportTracks(tracks)
    }

    private fun reportPause() {
        val paused = player?.playWhenReady == false
        if (lastPaused == paused) return
        lastPaused = paused
        property("pause", paused)
    }

    private fun durationSeconds(p: ExoPlayer): Double {
        // Live streams have no duration, as in mpv.
        if (p.isCurrentMediaItemLive || p.duration == C.TIME_UNSET) return 0.0
        return p.duration / 1000.0
    }

    private fun tick() {
        val p = player ?: return
        if (p.playbackState == Player.STATE_IDLE) return
        val time = p.currentPosition / 1000.0
        if (Math.abs(time - lastTime) >= 0.25) {
            lastTime = time
            property("time-pos", time)
        }
        val duration = durationSeconds(p)
        if (duration != lastDuration) {
            lastDuration = duration
            property("duration", duration)
        }
        val cache = p.bufferedPosition / 1000.0
        if (Math.abs(cache - lastCache) >= 1.0) {
            lastCache = cache
            property("demuxer-cache-time", cache)
        }
    }

    private fun reportTracks(tracks: Tracks) {
        val list = JSONArray()
        val next = HashMap<String, Int>()
        val map = HashMap<Pair<String, Int>, Pick>()
        for (group in tracks.groups) {
            val type = when (group.type) {
                C.TRACK_TYPE_VIDEO -> "video"
                C.TRACK_TYPE_AUDIO -> "audio"
                C.TRACK_TYPE_TEXT -> "sub"
                else -> continue
            }
            for (i in 0 until group.length) {
                if (!group.isTrackSupported(i)) continue
                val format = group.getTrackFormat(i)
                val id = (next[type] ?: 0) + 1
                next[type] = id
                map[Pair(type, id)] = Pick(type, group, i)
                val item = JSONObject().put("id", id).put("type", type).put("selected", group.isTrackSelected(i))
                format.language?.takeIf { it != "und" }?.let { item.put("lang", it) }
                format.label?.let { item.put("title", it) }
                format.sampleMimeType?.substringAfter('/')?.let { item.put("codec", it) }
                if (type == "audio" && format.channelCount > 0) item.put("demux-channel-count", format.channelCount)
                list.put(item)
            }
        }
        picks = map
        property("track-list", list)
    }

    private fun applyVolume() {
        player?.volume = if (muted) 0f else volume / 100f
    }

    private fun onMain(invoke: Invoke, block: () -> Unit) {
        main.post {
            try {
                block()
                invoke.resolve()
            } catch (e: Exception) {
                invoke.reject(e.message ?: e.toString())
            }
        }
    }

    @Command
    fun setEvents(invoke: Invoke) {
        events = invoke.parseArgs(EventsArgs::class.java).channel
        invoke.resolve()
    }

    @Command
    fun play(invoke: Invoke) {
        val args = invoke.parseArgs(PlayArgs::class.java)
        onMain(invoke) {
            val headers = HashMap(args.headers ?: emptyMap())
            val agent = headers.remove("User-Agent")?.takeIf { it.isNotBlank() && it != "libmpv" } ?: "Nightglass"
            headers.remove("Referer")?.takeIf { it.isNotBlank() }?.let { headers["Referer"] = it }
            val http = DefaultHttpDataSource.Factory()
                .setUserAgent(agent)
                .setAllowCrossProtocolRedirects(true)
                .setConnectTimeoutMs(20_000)
                .setReadTimeoutMs(20_000)
                .setDefaultRequestProperties(headers)
            val extractors = DefaultExtractorsFactory().setTsExtractorFlags(
                DefaultTsPayloadReaderFactory.FLAG_ALLOW_NON_IDR_KEYFRAMES or DefaultTsPayloadReaderFactory.FLAG_DETECT_ACCESS_UNITS,
            )
            val source = DefaultMediaSourceFactory(activity, extractors).setDataSourceFactory(http)
                .createMediaSource(MediaItem.fromUri(args.url))

            // Per-file options, written as mpv's: `start=93,aid=auto,sid=no`.
            val options = (args.options ?: "").split(',').filter { it.contains('=') }.associate {
                it.substringBefore('=') to it.substringAfter('=')
            }
            val p = ensurePlayer()
            textOff = options["sid"] == "no"
            applyText()
            lastState = Player.STATE_IDLE
            lastTime = Double.NEGATIVE_INFINITY
            lastDuration = Double.NEGATIVE_INFINITY
            lastCache = Double.NEGATIVE_INFINITY
            val start = options["start"]?.toDoubleOrNull()
            if (start != null && start > 0) p.setMediaSource(source, (start * 1000).toLong()) else p.setMediaSource(source)
            p.prepare()
            p.playWhenReady = true
            lastPaused = null
            reportPause()
            property("cache-start", 0.0)
        }
    }

    @Command
    fun command(invoke: Invoke) {
        val args = invoke.parseArgs(CommandArgs::class.java).args
        onMain(invoke) {
            val p = player
            when (args.firstOrNull()) {
                "stop" -> {
                    p?.stop()
                    p?.clearMediaItems()
                    lastState = Player.STATE_IDLE
                    property("track-list", JSONArray())
                }
                "cycle" -> if (args.getOrNull(1) == "pause" && p != null) {
                    p.playWhenReady = !p.playWhenReady
                    reportPause()
                }
                "seek" -> if (p != null && p.isCurrentMediaItemSeekable) {
                    val amount = (args.getOrNull(1)?.toDoubleOrNull() ?: 0.0) * 1000
                    val mode = args.getOrNull(2) ?: "relative"
                    val target = if (mode.startsWith("absolute")) amount else p.currentPosition + amount
                    p.seekTo(Math.max(0.0, target).toLong())
                }
            }
        }
    }

    @Command
    fun set(invoke: Invoke) {
        val args = invoke.parseArgs(SetArgs::class.java)
        onMain(invoke) {
            when (args.name) {
                "volume" -> {
                    volume = args.value.toDoubleOrNull()?.toInt()?.coerceIn(0, 100) ?: volume
                    applyVolume()
                }
                "mute" -> {
                    muted = args.value == "yes"
                    applyVolume()
                }
                "aid", "sid" -> pickTrack(args.name, args.value)
                "alang" -> editParameters { it.setPreferredAudioLanguage(args.value.ifBlank { null }) }
                "slang" -> editParameters { it.setPreferredTextLanguage(args.value.ifBlank { null }) }
                "stream-record" -> if (args.value.isNotEmpty()) throw IllegalStateException("Recording is not available on Android TV.")
            }
        }
    }

    private fun editParameters(change: (androidx.media3.common.TrackSelectionParameters.Builder) -> Unit) {
        val p = player ?: return
        p.trackSelectionParameters = p.trackSelectionParameters.buildUpon().also(change).build()
    }

    private fun applyText() = editParameters { it.setTrackTypeDisabled(C.TRACK_TYPE_TEXT, textOff) }

    private fun pickTrack(name: String, value: String) {
        val type = if (name == "aid") "audio" else "sub"
        if (type == "sub") {
            textOff = value == "no"
            applyText()
        }
        val id = value.toIntOrNull() ?: return
        val pick = picks[Pair(type, id)] ?: return
        editParameters {
            it.setOverrideForType(TrackSelectionOverride(pick.group.mediaTrackGroup, pick.index))
            if (type == "sub") it.setTrackTypeDisabled(C.TRACK_TYPE_TEXT, false)
        }
    }

    /** A web address in the system's own app: a YouTube trailer opens the YouTube app on a TV. */
    @Command
    fun openUrl(invoke: Invoke) {
        val args = invoke.parseArgs(UrlArgs::class.java)
        onMain(invoke) {
            val intent = android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(args.url))
                .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
            activity.startActivity(intent)
        }
    }

    @Command
    fun place(invoke: Invoke) {
        val a = invoke.parseArgs(PlaceArgs::class.java)
        onMain(invoke) {
            val v = view ?: return@onMain
            if (a.w <= 0 || a.h <= 0) {
                v.visibility = View.GONE
                return@onMain
            }
            val lp = (v.layoutParams as? FrameLayout.LayoutParams) ?: FrameLayout.LayoutParams(a.w, a.h)
            lp.width = a.w
            lp.height = a.h
            lp.leftMargin = a.x
            lp.topMargin = a.y
            lp.gravity = android.view.Gravity.TOP or android.view.Gravity.START
            v.layoutParams = lp
            v.visibility = View.VISIBLE
        }
    }
}
