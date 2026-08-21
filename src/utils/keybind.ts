const MODIFIER_LABELS: Record<string, string> = {
   ctrl: 'Ctrl',
   alt: 'Alt',
   shift: 'Shift',
};

const KEY_LABELS: Record<string, string> = {
   UP: 'Arrow Up',
   DOWN: 'Arrow Down',
   LEFT: 'Arrow Left',
   RIGHT: 'Arrow Right',
   SPACE: 'Space',
   RETURN: 'Enter',
   ENTER: 'Enter',
   ESCAPE: 'Escape',
   ESC: 'Escape',
   TAB: 'Tab',
   BACK: 'Backspace',
   BACKSPACE: 'Backspace',
   DELETE: 'Delete',
   DEL: 'Delete',
   INSERT: 'Insert',
   HOME: 'Home',
   END: 'End',
   PRIOR: 'Page Up',
   NEXT: 'Page Down',
   PAGEUP: 'Page Up',
   PAGEDOWN: 'Page Down',
   CAPITAL: 'Caps Lock',
   NUMLOCK: 'Num Lock',
   SCROLL: 'Scroll Lock',
   LWIN: 'Windows',
   RWIN: 'Windows',
   APPS: 'Menu',
   PAUSE: 'Pause',
   SNAPSHOT: 'Print Screen',
   CONTROL: 'Ctrl',
   LCONTROL: 'Left Ctrl',
   RCONTROL: 'Right Ctrl',
   SHIFT: 'Shift',
   LSHIFT: 'Left Shift',
   RSHIFT: 'Right Shift',
   MENU: 'Alt',
   LMENU: 'Left Alt',
   RMENU: 'Right Alt',
   MULTIPLY: 'Numpad *',
   ADD: 'Numpad +',
   SUBTRACT: 'Numpad -',
   DECIMAL: 'Numpad .',
   DIVIDE: 'Numpad /',
   // The OEM keys are labelled with the character they print, not their code name: a mod writing
   // "VK_OEM_6" means the ] key, and "Oem 6" told you nothing about what to press. These are the
   // US layout's characters — the fallback below has no way to supply them, since the name carries
   // no hint of the character. The label is display-only and never written back to an ini, so on a
   // layout that prints something else here the cost is a wrong character on screen, not a wrong
   // keybind. Reading the live layout instead would mean MapVirtualKeyW(vk, MAPVK_VK_TO_CHAR) on the
   // Rust side, and a name-to-code table there regardless, as Windows has no API that parses a VK
   // name.
   OEM_1: ';',
   OEM_PLUS: '=',
   OEM_COMMA: ',',
   OEM_MINUS: '-',
   OEM_PERIOD: '.',
   OEM_2: '/',
   OEM_3: '`',
   OEM_4: '[',
   OEM_5: '\\',
   OEM_6: ']',
   OEM_7: "'",
   // OEM_8 and OEM_102 print different characters on different boards, so they keep the fallback.
};

for (let i = 1; i <= 24; i++) KEY_LABELS[`F${i}`] = `F${i}`;
for (let i = 0; i <= 9; i++) KEY_LABELS[`NUMPAD${i}`] = `Numpad ${i}`;

function titleCaseFallback(token: string): string {
   return token
      .toLowerCase()
      .split(/[_\s]+/)
      .filter(Boolean)
      .map((word) => word[0].toUpperCase() + word.slice(1))
      .join(' ');
}

/** 3DMigoto ini key syntax ("ctrl VK_UP", "no_modifiers UP", "h") -> a human label
 * ("Ctrl + Arrow Up", "Arrow Up", "H"), for display only — never sent back to the ini. */
export function formatKeybind(raw: string): string {
   const tokens = raw.trim().split(/\s+/).filter(Boolean);
   if (tokens.length === 0) return raw;

   const keyToken = tokens[tokens.length - 1];
   if (keyToken.toLowerCase() === 'no_modifiers') return raw;

   const parts: string[] = [];
   for (const token of tokens.slice(0, -1)) {
      const label = MODIFIER_LABELS[token.toLowerCase().replace(/^vk_/i, '')];
      if (label) parts.push(label);
   }

   const withoutVk = keyToken.toLowerCase().replace(/^vk_/i, '');
   const upper = withoutVk.toUpperCase();
   const keyLabel =
      KEY_LABELS[upper] ??
      (withoutVk.length === 1 ? withoutVk.toUpperCase() : titleCaseFallback(withoutVk));

   parts.push(keyLabel);
   return parts.join(' + ');
}
