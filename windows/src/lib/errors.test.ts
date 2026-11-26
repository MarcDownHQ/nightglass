import { beforeAll, describe, expect, it } from 'vitest';
import { errorText, friendly, promptOf, protectedOff } from './errors';
import { i18n } from './i18n.svelte';

describe('friendly', () => {
  beforeAll(() => i18n.set('en'));

  it.each([
    ['Identifiants refusés par le serveur.', /username or password/],
    ['The server refused the credentials.', /username or password/],
    ['The server answered 403.', /too many devices/],
    ['Connection failed: error sending request', /can’t be reached/],
    ['Le serveur a répondu 403.', /too many devices/],
    ['Le serveur a répondu 404.', /nothing at this address/],
    ['Le serveur a répondu 502.', /failing/],
    ['Connexion impossible : error sending request: client error (Connect): dns error', /can’t be reached/],
    ['loading failed', /didn’t play/],
    ['unrecognized file format', /video stream/],
    ['Invalid URL', /isn’t valid/],
    ['Invalid path: list.m3u', /isn’t valid/],
    ['Missing or invalid setting: username.', /setting of this source/],
    ['No provider "acme" is installed.', /plugin that isn’t installed/],
    ['The plugin failed: wasm trap: unreachable', /plugin of this source failed/],
  ])('%s', (raw, expected) => expect(friendly(raw)).toMatch(expected));

  it('speaks French when asked', () => {
    i18n.set('fr');
    expect(friendly('loading failed')).toMatch(/hors ligne/);
    i18n.set('en');
  });

  it('passes unknown messages through', () => {
    expect(friendly('Something else')).toBe('Something else');
  });
});

describe('prompts', () => {
  beforeAll(() => i18n.set('en'));
  const prompt = { message: 'Sign in on another device', url: 'https://tv.test/activate', code: 'WDJB-MJHT', retryIn: 5 };

  it('tells a plugin waiting for the user from an error', () => {
    expect(promptOf({ prompt })).toEqual(prompt);
    expect(promptOf('The server answered 403.')).toBeNull();
    expect(promptOf(null)).toBeNull();
    expect(promptOf(new Error('boom'))).toBeNull();
  });

  it('says a prompt with its address and code, and other errors as they are', () => {
    expect(errorText({ prompt })).toBe(`Sign in on another device · ${prompt.url} · code WDJB-MJHT`);
    expect(errorText({ prompt: { ...prompt, url: null, code: null } })).toBe('Sign in on another device');
    expect(errorText('The server answered 403.')).toBe('The server answered 403.');
  });
});

describe('protected content', () => {
  beforeAll(() => i18n.set('en'));

  it('tells an encrypted stream waiting for the setting from an error', () => {
    expect(protectedOff({ protected: 'off' })).toBe(true);
    expect(protectedOff('Protected playback failed: not implemented yet')).toBe(false);
    expect(protectedOff({ prompt: { message: 'Sign in' } })).toBe(false);
    expect(protectedOff(null)).toBe(false);
  });

  it('says where to turn it on', () => {
    expect(errorText({ protected: 'off' })).toMatch(/turn on protected content in Settings/);
  });
});
