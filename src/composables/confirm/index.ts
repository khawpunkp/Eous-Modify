import { readonly, ref } from 'vue';

export interface ConfirmOptions {
   title: string;
   message: string;
   /** Wording of the button that goes ahead with the action. */
   confirmLabel?: string;
   /** Styles the confirm button as a dangerous action rather than a neutral one. */
   destructive?: boolean;
}

interface PendingConfirm extends ConfirmOptions {
   resolve: (confirmed: boolean) => void;
}

const pending = ref<PendingConfirm | null>(null);

/** The request ConfirmDialog is showing, if any. Answer it through `resolveConfirm`, not by writing here. */
export const pendingConfirm = readonly(pending);

/**
 * Asks the user to confirm something, resolving `true` only from the confirm button — cancelling,
 * clicking the backdrop and Escape all resolve `false`, so every way out of the dialog reads as "no"
 * and callers can keep the `if (!(await confirmAction(…))) return;` shape window.confirm() had.
 *
 * State lives at module level rather than in each caller so one host component renders every
 * request, which is what lets the call site stay a single expression.
 */
export function confirmAction(options: ConfirmOptions): Promise<boolean> {
   // A request arriving while another is on screen would otherwise strand the first promise forever:
   // its caller is awaiting a dialog that no longer exists.
   pending.value?.resolve(false);

   return new Promise<boolean>((resolve) => {
      pending.value = { ...options, resolve };
   });
}

/** Settles the request on screen — this is what resolves the promise `confirmAction` handed back. */
export function resolveConfirm(confirmed: boolean) {
   const request = pending.value;
   if (!request) return;
   // Cleared before resolving, so a caller that asks again the moment it wakes up isn't taken back
   // off screen by this call.
   pending.value = null;
   request.resolve(confirmed);
}
