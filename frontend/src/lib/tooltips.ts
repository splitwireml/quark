// Native popovers escape scrolling/overflow ancestors without a portal dependency.
export function floatingTooltips(node: HTMLElement) {
  const tip = document.createElement('div');
  tip.className = 'floating-tooltip';
  tip.popover = 'manual';
  tip.id = `tooltip-${crypto.randomUUID()}`;
  tip.setAttribute('role', 'tooltip');
  node.append(tip);
  let timer: ReturnType<typeof setTimeout>;
  let anchor: HTMLElement | null = null;
  let describedBy: string | null = null;

  function hide() {
    clearTimeout(timer);
    tip.hidePopover();
    if (anchor) {
      if (describedBy === null) anchor.removeAttribute('aria-describedby');
      else anchor.setAttribute('aria-describedby', describedBy);
    }
    anchor = null;
  }

  function show(event: Event) {
    const target = (event.target as Element).closest<HTMLElement>('[data-tip]');
    if (!target || !node.contains(target) || target.getAttribute('aria-expanded') === 'true') return;
    if (target === anchor) return;
    hide();
    anchor = target;
    describedBy = target.getAttribute('aria-describedby');
    timer = setTimeout(() => {
      if (!target.isConnected) return;
      tip.textContent = target.dataset.tip ?? '';
      tip.showPopover();
      target.setAttribute('aria-describedby', [describedBy, tip.id].filter(Boolean).join(' '));
      const rect = target.getBoundingClientRect();
      const bounds = tip.getBoundingClientRect();
      const top = rect.top - bounds.height - 8;
      tip.style.left = `${Math.max(8, Math.min(innerWidth - bounds.width - 8, rect.left + (rect.width - bounds.width) / 2))}px`;
      tip.style.top = `${Math.max(8, Math.min(innerHeight - bounds.height - 8, top >= 8 ? top : rect.bottom + 8))}px`;
    }, 260);
  }

  function leave(event: Event) {
    if (anchor?.contains((event as MouseEvent).relatedTarget as Node | null)) return;
    hide();
  }
  function escape(event: KeyboardEvent) { if (event.key === 'Escape') hide(); }
  node.addEventListener('pointerover', show);
  node.addEventListener('focusin', show);
  node.addEventListener('pointerout', leave);
  node.addEventListener('focusout', leave);
  node.addEventListener('pointerdown', hide);
  node.addEventListener('scroll', hide, true);
  window.addEventListener('keydown', escape);
  window.addEventListener('resize', hide);
  return () => {
    hide();
    tip.remove();
    node.removeEventListener('pointerover', show);
    node.removeEventListener('focusin', show);
    node.removeEventListener('pointerout', leave);
    node.removeEventListener('focusout', leave);
    node.removeEventListener('pointerdown', hide);
    node.removeEventListener('scroll', hide, true);
    window.removeEventListener('keydown', escape);
    window.removeEventListener('resize', hide);
  };
}
