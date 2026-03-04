---
name: visual-hierarchy-ui-aesthetics
description: Applies visual hierarchy, readability, and UI color best practices for aesthetically pleasing interfaces. Use when reviewing or designing UI for layout, typography, color, contrast, grouping, and scannability; complements usability-heuristics and baseline-ui for full UI/UX review.
---

# Visual Hierarchy & UI Aesthetics

Guides UI toward clear visual hierarchy, readable content, and effective color use. Sources: [Visme](https://visme.co/blog/visual-hierarchy/), [Nielsen Norman Group](https://www.nngroup.com/articles/visual-hierarchy-ux-definition/), [IxDF Visual Hierarchy](https://www.interaction-design.org/literature/topics/visual-hierarchy), [Tubik Studio](https://blog.tubikstudio.com/visual-hierarchy-effective-ui-content-organization/), [Tubik Readable UI](https://blog.tubikstudio.com/ux-design-readable-user-interface/), [IxDF UI Color Palette](https://www.interaction-design.org/literature/article/ui-color-palette).

## When to Use

- **UI/UX reviewer flows:** Invoke this skill when running or supporting ui-ux-reviewer tasks (accessibility, usability heuristics, motion, layout, design consistency). It adds visual hierarchy, readability, and color-palette criteria to the review.
- UI/UX review: layout, emphasis, and visual order
- Designing or refining screens for scannability and clarity
- Choosing or auditing color palettes and contrast
- Improving typography, spacing, and content structure
- Working alongside usability-heuristics and baseline-ui

---

## 1. Visual Hierarchy

**Definition:** Organize elements so the eye is guided to consume each in order of intended importance. If users struggle to know where to look first, hierarchy is weak.

### Tools (apply deliberately; overuse flattens hierarchy)

| Tool | Rule |
|------|------|
| **Size** | Larger = more important. Use at most 3 sizes (e.g. header, subheader, body). Make the most important element biggest; limit to ~2 big elements so they stand out. |
| **Scale** | One object has no scale until compared to another. Use scale to create balance and focus on dominant elements. |
| **Color & contrast** | Bright/saturated draws attention; muted recedes. Contrast in value and saturation with context matters more than the raw color. Don’t rely on color alone (accessibility). |
| **Typography** | Vary size, weight, spacing. Primary = headlines (biggest); secondary = subheaders/captions (scannable); tertiary = body/supporting. On mobile, consider 2 levels only. |
| **Negative space** | Space around content singles it out. More space = perceived as one group and often more emphasis. Clutter reduces clarity. |
| **Proximity** | Related items close together = perceived as a group. Use to chunk content (e.g. label + field, heading + paragraph). |
| **Alignment** | Shared margins and alignment create order and direct the eye. Left alignment supports F-pattern; center for simple, balanced layouts. |
| **Repetition** | Same style (font, color, shape) suggests related content and unity. Use consistently for similar roles (e.g. all links, all section titles). |
| **Common region** | Borders or backgrounds group elements explicitly when proximity isn’t enough; use sparingly to avoid clutter. |

### Scanning patterns

- **F-pattern:** Text-heavy; top horizontal scan, then down the left, scanning right when something relevant appears.
- **Z-pattern:** Image- or CTA-heavy; top left → top right → diagonal → bottom left → bottom right.
- Place key information along these paths; break the pattern deliberately for a single strong focal point (e.g. centered stat with space).

### Squint / blur test

Squint or apply a light blur (e.g. 5–10px). What stands out? If it’s not the intended primary focus, adjust size, contrast, or grouping. Strong content (e.g. a vivid image) can override template hierarchy—consider content when judging.

---

## 2. Readability & Legibility

**Legibility:** How well users see and distinguish elements (characters, words, blocks).  
**Readability:** Ease of understanding and scanning (structure, simplicity, clarity). Both matter for UI.

### Background & contrast

- Background color affects perceived size and clarity (e.g. black on light can look larger than white on dark). Ensure sufficient contrast (WCAG: ≥4.5:1 normal text, ≥3:1 large text).
- Avoid absolute white/black for large areas; prefer off-white/off-black to reduce strain. Test in different lighting.

### Typography for readability

- **White space:** Space between elements (and line/letter spacing) directly affects readability. Too little = tense, hard to scan.
- **Line length:** Restrain characters per line for body text so blocks stay digestible.
- **Hierarchy:** Headline → subheader → body. One main idea per block; short paragraphs.
- **Lists:** Use bullets or numbers to break up text and improve scannability.
- **Emphasis:** Bold, italic, or color for key phrases; use sparingly. Links must be visually distinct (e.g. color + underline or weight).
- **Numbers:** Numerals often attract fixations during scanning; use for facts and stats where appropriate.
- **Consistency:** One term per concept (e.g. “Delete” everywhere, not “Remove” on some screens). Short, consistent labels.

### Copy

- Clear, concise, useful, consistent. Prefer “Pay” over “Make a payment” for buttons. Support scanning with headlines that state the insight (e.g. “58% drop at checkout” not just “Key findings”).

---

## 3. Color & UI Palettes

- **Role of color:** Create hierarchy, group elements, signal importance, and support brand. Contrast with context drives attention more than the hue alone.
- **Primary:** Most frequent, guides to key UI and brand (e.g. main CTAs, key nav).
- **Secondary:** Supports structure and variety.
- **Accent:** Highlights important actions or states; use sparingly.
- **60-30-10:** Rough balance—60% dominant, 30% secondary, 10% accent (guideline, not rigid).
- **Limit variety:** In common UIs, 2 primary + 2 secondary colors reduce overwhelm. No more than ~3 contrast levels for type (e.g. header, subheader, body).
- **Accessibility:** Sufficient contrast; don’t convey meaning by color alone (add labels, icons, or patterns). Test in grayscale. Avoid combinations that are problematic for color vision.
- **Warm vs cool:** On dark backgrounds, warm colors advance, cool recede; on light, the opposite. Use to suggest depth if needed.

---

## 4. Review Checklist

Use when auditing a screen or flow for visual hierarchy and aesthetics:

**Hierarchy**
- [ ] One clear focal point or ordered path (size/contrast/position).
- [ ] At most 3 type sizes; most important element is largest.
- [ ] No more than ~2 “big” elements competing for attention.
- [ ] Related items grouped by proximity or common region; spacing between groups clear.

**Readability**
- [ ] Sufficient contrast (WCAG); no color-only meaning.
- [ ] Typography hierarchy (headline / subheader / body) clear.
- [ ] Adequate negative space; line length and spacing comfortable.
- [ ] Lists or structure for dense copy; key info easy to scan.
- [ ] Labels and copy consistent and concise.

**Color**
- [ ] Limited palette (e.g. 2 primary, 2 secondary); accent used sparingly.
- [ ] Primary actions (e.g. main CTA) clearly emphasized.
- [ ] No absolute white/black full-screen if it causes strain; accessibility checked.

**Consistency**
- [ ] Same style for same roles (e.g. all section titles, all links).
- [ ] Alignment and spacing consistent across the view.

---

## 5. Quick Fixes

- **Everything looks equal:** Increase size or weight of the single most important element; add space around it; reduce emphasis on secondary elements.
- **Cluttered:** Increase negative space between groups; reduce number of strong colors or font weights; one idea per block.
- **Hard to scan:** Add subheadings, bullets, or numbers; shorten paragraphs; strengthen typographic hierarchy.
- **Color chaos:** Reduce to 2 primary + 2 secondary; use one accent for CTAs; check contrast.
- **Weak focal point:** Use squint/blur test; adjust size, contrast, or position so the intended focal point wins.

When reporting, name the principle (e.g. “Visual hierarchy – proximity”) and give a short, actionable change. Use with usability-heuristics and baseline-ui for full UI/UX reviews.
