import { getCurrentWebview } from '@tauri-apps/api/webview';
import { prefs } from './prefs';

export type StartScreen = 'home' | 'live' | 'last';

/** The Settings window's choices. */
class Options {
  /** Where the app opens: Home, Live, or Live playing the last channel. */
  startScreen = $state<StartScreen>(prefs.json('startScreen', 'home'));
  startFullscreen = $state<boolean>(prefs.json('startFullscreen', false));
  /** Size of the whole interface, 1 = 100 %. For a TV across the room. */
  zoom = $state<number>(prefs.json('zoom', 1));
  /** Hours between two automatic reloads of the catalogs and guides while the app runs; 0: at launch only. */
  refreshHours = $state<number>(prefs.json('refreshHours', 12));
  /** Live: the picture as large as the window allows, the channel list still beside it. */
  cinema = $state<boolean>(prefs.json('cinema', false));

  set<K extends 'startScreen' | 'startFullscreen' | 'zoom' | 'refreshHours' | 'cinema'>(key: K, value: (typeof this)[K]) {
    this[key] = value;
    prefs.setJson(key, value);
    if (key === 'zoom') void this.applyZoom();
  }

  /** ±10 % steps, between 70 % and 200 %. */
  stepZoom(delta: number) {
    this.set('zoom', Math.round(Math.min(2, Math.max(0.7, this.zoom + delta)) * 10) / 10);
  }

  async applyZoom() {
    try {
      await getCurrentWebview().setZoom(this.zoom);
    } catch (e) {
      console.warn(`zoom: ${e}`);
    }
  }
}

export const options = new Options();
