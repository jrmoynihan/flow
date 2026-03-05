---
name: usability-heuristics
description: Evaluate and improve interfaces using usability heuristics from Stanford, Nielsen, Visily, and Shopify. Use when reviewing screens or flows, improving UX, auditing an app, or ensuring user control, clarity, consistency, feedback, error handling, forgiveness, and accessibility.
---

# Usability Heuristics

Apply industry-standard usability principles when reviewing or improving interfaces. Sources: [Stanford Usability Principles](https://improvement.stanford.edu/resources/usability-principles), [Visily UX principles](https://www.visily.ai/blog/ux-design-principles/), [Shopify UI principles](https://www.shopify.com/blog/ui-design-principles).

## When to Use This Skill

- Reviewing a screen, flow, or full app for usability
- Improving UX of existing UI (forms, navigation, dialogs)
- Auditing before release or for backlog improvements
- Checking user control, clarity, consistency, feedback, errors, and accessibility

## The 12 Heuristics (Stanford + common extensions)

### 1. User control and freedom

Users should feel in control and know where they are. Support undo and clear exit.

- [ ] User can undo or reverse critical actions (e.g. undo, revert).
- [ ] No automatic redirects without user action or clear notice.
- [ ] Breadcrumbs or context shows current location in a stepped process.
- [ ] "Cancel" or "Back" exits without committing; destructive actions are confirmable.
- [ ] Customization where it helps (e.g. list vs grid, saved preferences).

### 2. Recognition over recall

Reduce memory load: show information in the UI instead of asking users to remember it.

- [ ] Previously entered or known data is shown (e.g. autofill, "you entered X").
- [ ] Options are visible or easy to discover (menus, suggestions) rather than memorized.
- [ ] Multi-step flows show what was already submitted when confirming.

### 3. Mental model

The system should match how users think about the task and speak their language.

- [ ] Language and structure match the user's domain (no unnecessary jargon).
- [ ] Grouping reflects real-world structure (e.g. by building, then room).
- [ ] Behavior is predictable from labels and layout.

### 4. Clarity

Communicate clearly and efficiently.

- [ ] Labels, links, and menu items are clear and concise.
- [ ] Purpose of the page or step is obvious from copy and layout.
- [ ] Icons or visuals are self-explanatory or have tooltips/labels.
- [ ] Differences between similar options are explicitly described.

### 5. Simplicity and aesthetic integrity

Less is more. Remove what doesn’t serve the user or the task.

- [ ] No flashy or redundant elements that distract from tasks.
- [ ] Simple language, readable typography, restrained color scheme.
- [ ] Negative space used so important elements stand out.
- [ ] Visual hierarchy supports the main task (e.g. primary action prominent).

### 6. Accuracy

The interface is free from errors and misleading information.

- [ ] No typos, wrong labels, or misleading copy.
- [ ] Calculations and displayed data are correct.
- [ ] Status and state reflect reality (e.g. saved, synced).

### 7. Error prevention and handling

Prevent errors where possible; when they occur, explain clearly and help recovery.

- [ ] Validation and constraints prevent invalid input where possible.
- [ ] In-context instructions for non-obvious fields.
- [ ] Error messages in plain language and user terms (not raw codes).
- [ ] Guidance on how to fix the error (e.g. "Enter a valid email").

### 8. Consistency and predictability

Same things look and behave the same across the product.

- [ ] Same names for the same concepts (e.g. menu label matches page title).
- [ ] Stable placement of key elements (e.g. logo, search, primary nav).
- [ ] Buttons and links describe the outcome (e.g. "Submit order" not just "Go").
- [ ] Follow platform and web conventions so behavior is predictable.

### 9. User support

Help is available when needed.

- [ ] Contextual help (e.g. tooltips, "Why do we need this?") where useful.
- [ ] Clear path to support or documentation (e.g. help link, contact).
- [ ] Optional in-app or live help for complex flows.

### 10. Forgiveness (emergency exit)

Users can leave unwanted states and reverse actions without penalty.

- [ ] Clearly marked way to cancel or close (e.g. "Cancel", "Close", Esc).
- [ ] Destructive actions are reversible or confirmed.
- [ ] Edit after submit where appropriate; revision history when relevant.

### 11. Feedback

Users are informed about system state and the result of actions in reasonable time.

- [ ] Immediate feedback for actions (e.g. button state, success message).
- [ ] Progress or loading indicator for operations that take more than ~400 ms.
- [ ] Success confirmation for completion (e.g. "Saved", thank-you step).
- [ ] Validation feedback on fields (e.g. required, format) in context.

### 12. Accessibility

Design so people with disabilities can use the product; aim for WCAG 2 AA where applicable.

- [ ] Don’t rely on color alone (use label, icon, or pattern as well).
- [ ] Sufficient color contrast for text and interactive elements.
- [ ] Keyboard navigation and visible focus states.
- [ ] Images have appropriate alt text; structure (headings, landmarks) supports screen readers.

## Comfort and iteration (Shopify / Visily)

- [ ] **Comfort**: Minimal, clear layout; clear hierarchy; consistent, simple language; avoid jargon.
- [ ] **User control**: Prioritize user goals; support undo and safe exploration.
- **Plan for failure**: Error and empty states are clear and helpful; 404 or dead ends offer a path forward.
- **Iterate**: Treat design as iterative; use feedback and data to refine.

## Review workflow

1. **Scope**: Choose one flow or screen set (e.g. sign-up, checkout, settings).
2. **Checklist**: Walk through the 12 heuristics and comfort/control/errors above; tick what’s satisfied, note gaps.
3. **Prioritize**: Critical = blocks or confuses core task; Suggestion = improves clarity or efficiency; Nice-to-have = polish.
4. **Suggestions**: For each gap, suggest a concrete change (copy, layout, or interaction) and tie it to the heuristic.

When reporting, name the heuristic (e.g. "User control and freedom") and give a short, actionable recommendation.
