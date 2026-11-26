import { afterEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const { locked, widevine } = await import('./widevine.svelte');

describe('protected content', () => {
  afterEach(() => {
    invoke.mockReset();
    widevine.version = null;
    widevine.step = null;
  });

  it('greys out protected items only while it is off', () => {
    expect(locked({ protected: true })).toBe(true);
    expect(locked({ protected: false })).toBe(false);
    expect(locked({})).toBe(false);
    widevine.version = '4.10.3112.0';
    expect(locked({ protected: true })).toBe(false);
  });

  it('plays what was started once the module is in', async () => {
    const play = vi.fn();
    widevine.blocked('Locked', play);
    expect(widevine.step).toBe('blocked');
    invoke.mockResolvedValueOnce('4.10.3112.0');
    await widevine.install();
    expect(invoke).toHaveBeenCalledWith('widevine_install');
    expect(widevine.enabled).toBe(true);
    expect(widevine.step).toBeNull();
    expect(play).toHaveBeenCalledOnce();
  });

  it('says why an install failed, and plays nothing', async () => {
    const play = vi.fn();
    widevine.blocked('Locked', play);
    invoke.mockRejectedValueOnce('Connection failed: dns error');
    await widevine.install();
    expect(widevine.step).toBe('failed');
    expect(widevine.error).toMatch(/dns error/);
    expect(play).not.toHaveBeenCalled();
  });

  it('cancels a download when its window closes', async () => {
    let finish: (version: string) => void = () => {};
    invoke.mockImplementation((command: string) => (command === 'widevine_install' ? new Promise((resolve) => (finish = resolve)) : Promise.resolve(null)));
    const play = vi.fn();
    widevine.blocked('Locked', play);
    const installing = widevine.install();
    await Promise.resolve();
    widevine.close();
    finish('4.10.3112.0');
    await installing;
    expect(invoke).toHaveBeenCalledWith('widevine_remove');
    expect(play).not.toHaveBeenCalled();
  });
});
