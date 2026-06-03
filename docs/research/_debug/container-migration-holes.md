# Container Migration Holes

## A. Solver — кто кому считает rect

**Файл:** `uzor/src/layout/solve.rs:32-170`  
**Сейчас:** `solve_layout` принимает `&ChromeSlot`, `&EdgePanels`, `&mut LayoutTree`. Пишет ректы в `LayoutTree` узлы (`tree.set_rect(tree.chrome_id(), ...)`, `tree.set_rect(tree.edge_id(...), ...)`). `WindowBranch.containers` в функцию не передаётся. Ни одной итерации по `containers`. Ни одной записи в `Container.rect`.

`solve` / `solve_window` в `manager.rs:1475-1484` и `1177-1186` вызывают `solve_layout` + `b.dock.layout(dock_pr)`. Ни после, ни внутри — нет цикла по `containers` с записью `c.rect`.

**Должно:** после solve прогонять все `containers` с `Placement::Edge` и `Placement::Dock` и писать итоговый `Container.rect` из `LayoutSolved` / dock-tree.

**Дыра №1 — ДА.** `Container.rect` для Edge и Dock контейнеров никогда не пересчитывается solver'ом.

---

## B. Где `Container.rect` пишется после register_container

Все write-сайты в `manager.rs`:

| Строка | Что |
|--------|-----|
| 1807 | `preserve_float` — копирует старый rect из себя же при повторном register |
| 1813 | `spec.rect` — initial register, пишет то что дал caller |
| 2249 | `apply_resize_drag` — Float only: `c.rect = r` |
| 2718 | `tear_off_to_float` — Float: `c.rect = r` |
| 2746 | `move_float_container` — Float drag move: `c.rect = r` |

**Сейчас:** `Container.rect` меняется только при Float-операциях и при re-register. Для Edge — остаётся `spec.rect` переданным при первом `register_container`, обычно 0×0 или placeholder.  
Для Dock — rect тоже не обновляется; dock-tree хранит ректы в своём `PanelRect`, но они не копируются в `Container.rect`.

**Дыра — ДА.** Нет ни одного места где Edge/Dock `Container.rect` = актуальный rect после solve.

---

## C. Hit zones обновление

**Файл:** `uzor/src/input/core/coordinator.rs:171-223`

`InputCoordinator::begin_frame_widgets_only` (строка 213) делает `self.widgets.clear()` и `self.layers.clear()` **каждый frame**.

`LayoutManager::begin_frame` (manager.rs:587) вызывает `ctx.begin_frame_widgets_only(...)` — все виджеты сносятся.

`register_container` (manager.rs:1767) регистрирует хиты через `coord.register_composite` и `coord.register_child` — но их ректы берутся из `rect` переменной (manager.rs:1833-1893), которая = `spec.rect` (или preserved float rect).

**Сейчас:** zones полностью очищаются каждый frame. Их регистрируют заново на каждый `register_container` вызов. Проблема: если `register_container` вызывается с `spec.rect = 0×0` (потому что Edge-контейнер не знает своего rect до solve, а solve не пишет в него), то hit-зоны и рисуются как 0×0, и `if !on || w <= 0.0 || h <= 0.0 { return; }` (строка 1846) — edge handles просто не создаются.

**Дыра №2 — ДА.** Hit zones корректно пересоздаются каждый frame, но их ректы неверны для Edge/Dock контейнеров, потому что `Container.rect` не пересчитан (дыра №1 → дыра №2 следствие).

---

## D. Click delivery — Float header_drag

**Файл:** `manager.rs:3015-3141`

`on_pointer_down` (строка 3015):
1. `b.ctx.input.process_drag_press(x, y)` — hit-test через InputCoordinator.
2. Если hit = `<wid>:header_drag` → строка 3123 `strip_suffix(":header_drag")`.
3. Resolve `widget_to_slot` → строка 3129-3138: проверяет `is_dock = matches!(placement, Placement::Dock { .. })`. Если `Dock` — создаёт `active_header_drag`.
4. Если `Float` — **ветка пропускается**. `is_dock = false`, `active_header_drag` не ставится.

Также dispatcher routes (строка 1900-1916): dispatcher не регистрирует route для `:header_drag` suffix — там только edge/corner variants. Комментарий строки 1756-1758: `"header drag forwards to a future ContainerHeaderDrag event (TODO)"`.

**Сейчас для Float контейнера:**
- Хит `header_drag` обнаруживается (если rect ненулевой).
- `on_pointer_down` не создаёт `active_header_drag` — `is_dock` guard отсекает Float.
- Dispatcher route для `:header_drag` не зарегистрирован — `DispatchEvent::Unhandled`.
- `on_pointer_move` header drag path (строки 2993-3012) требует `active_header_drag.is_some()` — не срабатывает.

**Дыра №3 — ДА.** Float контейнер с `header_drag` хитом не начинает drag — `is_dock` guard отсекает и route отсутствует.

---

## E. ChromeSlot vs containers["chrome"]

**ChromeSlot** (`branch.chrome: ChromeSlot`):
- Используется в: `solve_layout` (solve.rs:47) для вычисления chrome rect → `tree.set_rect(tree.chrome_id(), ...)`.
- `rect_for_chrome` (manager.rs:1498) читает из `last_solved.chrome` — из LayoutSolved, не из containers.
- Через legacy `lm.chrome()` / `lm.chrome_mut()` — приложение настраивает высоту и видимость.

**containers["chrome"]** (`Placement` неизвестна, регистрируется через `draw_all_containers` path):
- Используется в: `draw_all_containers` строка 2368 — `if slot == "chrome"` — рисует через `paint_chrome_from_content` из `ContainerContent::Chrome`.
- `Container.rect` для slot "chrome" = то что передал `register_container` при регистрации.

**Рендер chrome bar:** путь через `containers["chrome"]` → `draw_all_containers` → `paint_chrome_from_content`. `ChromeSlot` здесь не участвует в рисовании напрямую.

**Hit-test chrome controls:** через dispatcher routes (ChromeWindowControl pattern), виджеты регистрирует `ChromeElement::apply()` — это уже L3 путь через `containers["chrome"]`.

**Итог:** `ChromeSlot` — ведущий для **solver** (high/visible → LayoutSolved.chrome). `containers["chrome"]` — ведущий для **render** и **hit-test**. `Container.rect` для "chrome" не синхронизируется с `LayoutSolved.chrome`.

**Дыра — ДА (частично).** `Container["chrome"].rect` и `solved.chrome` — разные источники. Если rect в containers["chrome"] не проставляется из LayoutSolved после solve, рисование chrome идёт по неверному rect'у.

---

## F. Edge containers — почему rect 0×0

**Файл:** `solve.rs:60-157`

`solve_layout` итерирует `edges.slots_for(EdgeSide::*)` — это `&EdgePanels`, legacy структура. Результаты пишет в `LayoutTree` узлы (`tree.edge_id(side)`). Никакой итерации по `containers` c `Placement::Edge { side }` нет.

`apply_resize_drag` для `Placement::Edge` (manager.rs:2203-2226): мутирует `b.edges.get_mut(slot_id)` → `edge_slot.thickness`. Правильно пишет в legacy `EdgePanels`, потому что `solve_layout` читает оттуда. Но `Container.rect` по-прежнему не обновляется.

**Сейчас:** при каждом `register_container` с `Placement::Edge` — `Container.rect` = `spec.rect` из вызывающего кода. `solve` не копирует computed edge rect обратно. Функции `rect_for_chrome()`, edge rect accessors (строки 1498-1533) читают из `last_solved` — минуя `containers`.

Код "найти все containers с Placement::Edge → вычислить их rect → записать в `c.rect`" — **отсутствует**.

**Дыра №4 — ДА.** После solve нет шага back-propagation edge rects в `Container.rect`.

---

## G. Float интерактив — почему drag не работает

Три независимых причины:

**G1.** `register_container` для Float: `rect = preserved_rect` (если Float уже существует) или `spec.rect`. Hit зоны регистрируются по этому rect. Если initial `spec.rect` корректный — hit-zone создаётся верно. **Не блокер сам по себе.**

**G2.** Dispatcher route для `:header_drag` — не зарегистрирован. `register_container` строки 1900-1916 регистрирует только `edge_*` и `corner_*` routes. `:header_drag` route — TODO комментарий строка 1757. `dispatcher.on_exact("...:header_drag", ...)` нет.

**G3.** `on_pointer_down` строка 3130-3138: `active_header_drag` устанавливается **только если** `Placement::Dock`. Float-placement — игнорируется.

**Сейчас:** Float header drag: хит попадает в InputCoordinator (если rect ненулевой), но `on_pointer_down` гасит его на guard `is_dock`, `active_header_drag` = None, `on_pointer_move` header-drag ветка не входит.

**Дыра №3 = дыра G — ДА.** Float drag мёртв из-за двух причин: отсутствующий guard-branch в `on_pointer_down` + отсутствующий dispatcher route.
