# Design system

The Phase 1 reference is [`design/deskflow-phase1-concept.png`](design/deskflow-phase1-concept.png). The implemented UI keeps its graphite/slate surfaces, restrained electric-blue focus treatment, Segoe-like typography, and compact Windows-native geometry.

## Tokens

- dark background `#11161d`; elevated surface `#171d25`
- light background `#f8fafc`; elevated surface `#ffffff`
- accent `#2997ff` dark / `#087fe7` light
- text `#f2f6fb` dark / `#16202c` light
- 8–10px control radii, 12–16px window radii
- 120–140ms functional transitions; disabled by reduced-motion preference

## Rules

- One primary focus target in the overlay.
- No glassmorphism, gradients, decorative badges, fake metrics, or ornamental panels.
- UI controls and text remain code-native.
- Capability labels must state their real phase: Phase 2 captures local context, Phase 3 inspects a read-only UI tree, Phase 4 highlights targets, Phase 5 proposes validated plans, Phase 6 executes only after a second confirmation, and Phase 7 verifies each result with bounded recovery. The secure provider vault is a user-requested Phase 9 slice, not a claim that all Phase 8–9 work is complete. The UI must distinguish provider configuration from provider selection, and a proposed plan from active execution, verified completion, recovered completion, or a safe stop.
- API-key inputs are masked, never prefilled, and cleared after every save attempt. The UI may show only configured state and credential source, never secret values or key fragments.
- Icons use one 1.7px rounded-stroke family. The sparkle brand mark is the only filled symbol.
- Preserve keyboard-visible focus and WCAG-friendly contrast.

The reference shows a specific application name in the status line. The implementation intentionally replaces it with the truthful Phase 2 boundary because foreground-window detection is not part of Phase 1.
