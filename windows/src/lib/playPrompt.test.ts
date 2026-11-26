import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LiveItem, MovieItem, Prompt } from './models';
import { retryLater } from './signIn';

const invoke = vi.hoisted(() => vi.fn(async (_command: string, _args?: unknown): Promise<unknown> => null));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}), emitTo: vi.fn() }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ setFullscreen: vi.fn(), isFullscreen: vi.fn(async () => false), listen: vi.fn(async () => () => {}) }) }));

const { playPrompt } = await import('./playPrompt.svelte');
const { player } = await import('./player.svelte');
const { store } = await import('./store.svelte');

const prompt: Prompt = { message: 'Sign in on another device', url: 'https://tv.test/activate', code: 'WDJB-MJHT', retryIn: 5 };
const waiting = { prompt };
const source = { id: 's', name: 'S', provider: 'p', config: {} };
const channel = (name: string): LiveItem =>
  ({ id: `s:${name}`, source, channel: { id: name, name, number: null, group: null, logo: null, epgId: null, stream: `fr-tv:${name}`, headers: {} } }) as unknown as LiveItem;
const asked = (command: string) => invoke.mock.calls.filter(([c]) => c === command).length;
const answer = { url: 'http://127.0.0.1:9/live.m3u8', headers: {} };

describe('retryLater', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('asks again once after retry-in seconds, not after it is cancelled, never without them', () => {
    const retry = vi.fn();
    retryLater(5, retry);
    vi.advanceTimersByTime(4999);
    expect(retry).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(retry).toHaveBeenCalledOnce();
    retryLater(5, retry)();
    retryLater(null, retry);
    vi.advanceTimersByTime(60_000);
    expect(retry).toHaveBeenCalledOnce();
  });
});

describe('a plugin waiting for the user at play time', () => {
  let played: string[];

  beforeEach(() => {
    played = [];
    vi.spyOn(player, 'play').mockImplementation((p) => void played.push(p.title));
    vi.spyOn(player, 'expand').mockResolvedValue(undefined);
    store.providers = [{ id: 'p', name: 'P', fields: [], resolvesStreams: true, presets: [] }];
  });

  afterEach(() => {
    playPrompt.cancel();
    invoke.mockReset();
    invoke.mockImplementation(async () => null);
    vi.restoreAllMocks();
  });

  it('keeps the prompt on screen and plays as soon as the plugin answers', async () => {
    invoke.mockRejectedValueOnce(waiting);
    await store.tune(channel('M6'));
    expect(playPrompt.prompt).toEqual(prompt);
    expect(playPrompt.title).toBe('M6');
    expect(played).toEqual([]);

    invoke.mockRejectedValueOnce({ prompt: { ...prompt, code: 'NEW-CODE' } });
    await playPrompt.retry();
    expect(playPrompt.prompt?.code).toBe('NEW-CODE');
    expect(played).toEqual([]);

    invoke.mockResolvedValueOnce(answer);
    await playPrompt.retry();
    expect(playPrompt.prompt).toBeNull();
    expect(played).toEqual(['M6']);
    expect(asked('stream_url')).toBe(3);
  });

  it('stops when the user cancels, even with an answer on its way', async () => {
    invoke.mockRejectedValueOnce(waiting);
    await store.tune(channel('M6'));
    let resolve: (value: unknown) => void = () => {};
    invoke.mockReturnValueOnce(new Promise((r) => (resolve = r)));
    const retrying = playPrompt.retry();
    playPrompt.cancel();
    resolve(answer);
    await retrying;
    expect(played).toEqual([]);
    expect(playPrompt.prompt).toBeNull();
    await playPrompt.retry();
    expect(asked('stream_url')).toBe(2);
  });

  it('stops when another channel is tuned', async () => {
    invoke.mockRejectedValueOnce(waiting);
    await store.tune(channel('M6'));
    invoke.mockResolvedValueOnce(answer);
    await store.tune(channel('W9'));
    expect(playPrompt.prompt).toBeNull();
    expect(played).toEqual(['W9']);
  });

  it('closes on another error, which shows as before', async () => {
    invoke.mockRejectedValueOnce(waiting);
    await store.tune(channel('M6'));
    invoke.mockRejectedValueOnce('The server answered 403.');
    await playPrompt.retry();
    expect(playPrompt.prompt).toBeNull();
    expect(player.error).toBe('The server answered 403.');
  });

  it('does the same for a movie', async () => {
    const movie = { id: 'm', source, movie: { id: 'm', name: 'Film', group: null, poster: null, stream: 'fr-tv:m', rating: null, plot: null } } as unknown as MovieItem;
    invoke.mockRejectedValueOnce(waiting);
    await store.playMovie(movie);
    expect(playPrompt.title).toBe('Film');
    invoke.mockResolvedValueOnce(answer);
    await playPrompt.retry();
    expect(played).toEqual(['Film']);
    expect(playPrompt.prompt).toBeNull();
  });
});
