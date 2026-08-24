# UX recovery — why it felt broken, what we ship instead

## Why users bounced

| Report | Cause |
| --- | --- |
| No graphs | History is blank braille (`⠀`) padded on the left, dim on black. Looks empty. |
| No GPU | Never collected. |
| “P” + green bar | All cores tagged Performance. Unlabeled sparkline of cores. |
| Can’t select a process | List re-sorts every tick; selection is a row index. |
| Filter then stuck | `/` swallows every key including `?`. No visible search field. Esc is the only exit — never shown. |
| Kill does nothing | Bound to **Shift+K**. `k` is move up. Failures swallowed. |
| No network | Deferred. |
| Not clickable | Mouse capture on, no hit-testing. |

## vs macmon / btop

| | macmon | btop | us (before) | us (target) |
| --- | --- | --- | --- | --- |
| First 3s | silicon numbers | pretty graphs | empty + mystery P bar | **one labeled sparkline + live %** |
| GPU | yes | yes (AS) | no | IOAccelerator % |
| Processes | no | yes, followable | jumpy, trap filter | **pid-stable, always-on search** |
| Kill | no | signals | hidden Shift+K | **x + named confirm** |
| Net | no | yes | no | rx/tx in the strip |
| Small pane | native | ugly | accidental Glance | Glance still auto |
| Settings | interval only | huge menu | `[` `]` hidden | **s overlay** |

Better than macmon: act on processes, search, kill, net, mouse.  
Better than btop: Mac GPU %, honest labels, calmer default, search that doesn’t trap you.

## Journeys

1. **Glance** — open, read cpu/gpu/mem/net in one strip, leave.
2. **Find hog** — look at process table (doesn’t jump), click or `j/k`, `/` type, pick, `x`, `y`.
3. **Oops filter** — Esc clears, `?` always works, click Search to focus.
4. **Tune** — `s` interval + show/hide gpu/net/cores.

## Keys (v2)

| Key | Always |
| --- | --- |
| `?` | help |
| `q` | quit (not while typing in search — Esc first) |
| `s` | settings |
| `/` | focus search |
| `Esc` | close help/settings/confirm; else clear search |
| `j` `k` / arrows / wheel | move |
| `x` | kill selected (confirm y/n) |
| `g` `w` | glance / work |
| click | select row, focus search, footer actions |

## Layout (Work)

```
cpu  18%   ▁▂▃▅▇█▇▅▃▂▁▂▃▄   1s
gpu   4%   mem 22/36G ●   net ↓1.2M ↑0.3M   cores ▁▂█▄
────────────────────────────────
search  xcode_                      184 procs
 904  Xcode           48.1   4.2G
 312  WindowServer     5.2   312M
? help   / search   x kill   s settings   q quit
```

One sparkline. Every number labeled. Search always visible.
