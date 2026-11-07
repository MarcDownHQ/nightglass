import { describe, expect, it } from 'vitest';
import { friendly } from './errors';

describe('friendly', () => {
  it.each([
    ['Identifiants refusés par le serveur.', /username or password/],
    ['Le serveur a répondu 403.', /too many devices/],
    ['Le serveur a répondu 404.', /nothing at this address/],
    ['Le serveur a répondu 502.', /failing/],
    ['Connexion impossible : error sending request: client error (Connect): dns error', /can’t be reached/],
    ['loading failed', /didn’t play/],
    ['unrecognized file format', /video stream/],
  ])('%s', (raw, expected) => expect(friendly(raw)).toMatch(expected));

  it('passes unknown messages through', () => {
    expect(friendly('Something else')).toBe('Something else');
  });
});
