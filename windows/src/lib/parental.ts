import { t } from './i18n.svelte';
import { prompt } from './dialogs.svelte';
import { library } from './library.svelte';
import { player } from './player.svelte';

/**
 * Before changing what the PIN guards: makes one when there is none, else asks for it (once per
 * session). False when the user cancels or types it wrong.
 */
export async function unlock(): Promise<boolean> {
  if (!library.pinHash) {
    const pin = await prompt.ask(t('Choose a parental PIN'), '', { secret: true });
    if (!pin) return false;
    const again = await prompt.ask(t('Type the PIN again'), '', { secret: true });
    if (again !== pin) {
      player.notify(t('The two PINs differ'));
      return false;
    }
    await library.setPin(pin);
    library.unlocked = true;
    return true;
  }
  if (library.unlocked) return true;
  const pin = await prompt.ask(t('Parental PIN'), '', { secret: true });
  if (pin === null) return false;
  if (await library.checkPin(pin)) {
    library.unlocked = true;
    return true;
  }
  player.notify(t('Wrong PIN'));
  return false;
}

/** Puts a category behind the PIN, or takes it out. */
export async function toggleCategoryLock(key: string) {
  if (!(await unlock())) return;
  library.toggleLock(key);
  player.notify(library.lockedGroups.has(key) ? t('Locked: hidden until the PIN is typed') : t('Unlocked'));
}

/** Removes the PIN and every lock. */
export async function removePin() {
  if (!(await unlock())) return;
  for (const key of [...library.lockedGroups]) library.toggleLock(key);
  await library.setPin(null);
  library.unlocked = false;
  player.notify(t('PIN removed'));
}
