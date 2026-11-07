import { beforeAll, describe, expect, it } from 'vitest';
import { friendly } from './errors';
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
