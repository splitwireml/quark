<!-- Indeterminate progress along the bottom edge of a positioned parent. -->
<span class="bar" aria-hidden="true"></span>

<style>
  .bar { position: absolute; left: 0; right: 0; bottom: 0; height: 2px; overflow: hidden; }
  .bar::after {
    content: ''; position: absolute; inset: 0; width: 40%;
    background: var(--action);
    animation: busy-slide 1.1s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  }
  @keyframes busy-slide { from { transform: translateX(-100%); } to { transform: translateX(250%); } }
  @media (prefers-reduced-motion: reduce) { .bar::after { width: 100%; opacity: 0.6; animation: none; } }
</style>
