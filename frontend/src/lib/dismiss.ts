type DismissReason = 'escape' | 'outside';

/**
 * Closes a menu on Escape or a click outside it — never on hover-out. Clicks on
 * the control that opened the menu are left alone, so its own toggle decides.
 */
export function dismissable(node: HTMLElement, onDismiss: (reason: DismissReason) => void) {
  let dismiss = onDismiss;

  function onPointerDown(event: PointerEvent) {
    const target = event.target as HTMLElement | null;
    if (!target || node.contains(target) || target.closest?.('[data-menu-trigger]')) return;
    dismiss('outside');
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    event.stopPropagation();
    dismiss('escape');
  }

  window.addEventListener('pointerdown', onPointerDown);
  window.addEventListener('keydown', onKeydown, true);

  return {
    update(next: (reason: DismissReason) => void) { dismiss = next; },
    destroy() {
      window.removeEventListener('pointerdown', onPointerDown);
      window.removeEventListener('keydown', onKeydown, true);
    }
  };
}
