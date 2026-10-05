# Phase 1 fidelity ledger

Compared at original resolution with the reference [`../design/deskflow-phase1-concept.png`](../design/deskflow-phase1-concept.png), [`phase1-overlay-dark.png`](phase1-overlay-dark.png), and [`phase1-settings-dark.png`](phase1-settings-dark.png).

| Area | Reference | Render | Result |
| --- | --- | --- | --- |
| Overlay composition | compact title, single focus field, status, right-aligned key hints | same four-part hierarchy at 680×250 | matched |
| Palette | graphite/slate surfaces with one electric-blue focus color | sampled tokens preserve the same contrast and temperature | matched |
| Typography | Windows-native sans serif with compact control labels | Segoe UI Variable/Segoe UI stack with explicit weights/sizes | matched |
| Settings anatomy | title bar, five-item left rail, three General groups, footer actions | same structure at 940×720 without overflow | matched |
| Icons | restrained rounded-stroke navigation and filled sparkle mark | one custom 1.7px icon family plus filled sparkle | matched |
| Spacing and framing | open groups separated by rules; no nested card grid | same open panel model and group rhythm | matched |
| Responsive behavior | desktop reference only | 390px layout converts the rail to a scrollable, hidden-scrollbar tab strip | intentional extension |
| Motion | static reference | 140ms overlay entrance and 120ms controls, removed for reduced motion | intentional extension |

## Above-the-fold copy evolution

The brand name, command placeholder, Settings labels, General controls, and action labels are preserved. Two intentional deviations keep the UI aligned with implemented milestones:

- `Current app: Microsoft Excel` becomes the process name from the real Phase 2 foreground snapshot, with `No target captured` or `Target capture unavailable` when no valid observation exists.
- `Enter Analyze` becomes `Enter Stage`; submitted text moves to the truthful `Command staged · AI planning arrives in Phase 5` state.

The settings implementation adds short helper lines for shortcut semantics and opt-in startup. Phase 2 also turns Advanced into a diagnostics view; see [`PHASE2.md`](PHASE2.md).

## QA conclusion

No material visual mismatch remains at the native overlay or settings dimensions. The implementation preserves the reference's hierarchy, density, palette, container model, and icon treatment while clearly labeling unavailable later-phase features.
