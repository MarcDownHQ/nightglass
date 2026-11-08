import { describe, expect, it } from 'vitest';
import { displayName, qualityTag } from './theme';

describe('displayName', () => {
  it.each([
    ["Alpe d'Huez TV (720p) [Not 24/7]", "Alpe d'Huez TV"],
    ['FR | Info 24 HD', 'Info 24'],
    ['|FR| Sport 2', 'Sport 2'],
    ['Sport 1 FHD', 'Sport 1'],
    ['HD', 'HD'],
    ['TV5 Monde', 'TV5 Monde'],
  ])('%s → %s', (name, shown) => expect(displayName(name)).toBe(shown));
});

describe('qualityTag', () => {
  it.each([
    ['TF1 (1080p)', 'FHD'],
    ['TF1 720p', 'HD'],
    ['TF1 FHD', 'FHD'],
    ['TF1 uhd', '4K'],
    ['TF1 SD', 'SD'],
    ['TF1', null],
    ['HDMI Channel', null],
    ['VIP: RMC STORY ᴴᴰ', 'HD'],
    ['4K: V SPORT ᵁᴴᴰ ³⁸⁴⁰ᴾ', '4K'],
    ['RMC SPORT 2 ᴿᴬᵂ', null],
  ])('%s → %s', (name, tag) => expect(qualityTag(name)).toBe(tag));
});
