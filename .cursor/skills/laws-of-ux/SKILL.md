---
name: laws-of-ux
description: Apply cognitive and perception-based UI principles from Laws of UX. Use when evaluating layout, interaction design, target size and distance, choice count, information grouping, memory load, or response time. Cites specific laws (e.g. Fitts's Law, Hick's Law) to justify design decisions or suggest improvements.
---

# Laws of UX

Apply psychology- and cognition-based principles when reviewing or improving interfaces. Source: [Laws of UX](https://lawsofux.com/).

## When to Use This Skill

- Evaluating or suggesting changes to layout, buttons, navigation, or forms
- Explaining why a design feels slow, cluttered, or confusing
- Justifying design recommendations with named principles
- Checking interaction timing, target size, choice count, or information structure

## Core Laws (Quick Reference)

### Interaction & targets

| Law | Idea | Apply by |
|-----|------|----------|
| **Fitts's Law** | Time to acquire a target depends on distance and size. | Make important/primary actions larger and closer to the cursor or thumb; reduce distance for frequent actions. |
| **Hick's Law** | Decision time increases with number and complexity of choices. | Reduce options per step; use progressive disclosure; group or chunk choices. |
| **Doherty Threshold** | Productivity soars when system response is &lt;~400 ms. | Keep feedback and transitions under ~400 ms so the user doesn't wait; use loading states for longer operations. |

### Perception & grouping

| Law | Idea | Apply by |
|-----|------|----------|
| **Law of Proximity** | Nearby elements are perceived as grouped. | Place related controls/labels close together; add space between unrelated groups. |
| **Law of Common Region** | Elements in a bounded area are perceived as a group. | Use borders, cards, or background to group related content. |
| **Law of Similarity** | Similar-looking elements are perceived as related. | Use consistent style (e.g. same button style) for same type of action. |
| **Law of Uniform Connectedness** | Visually connected elements (lines, alignment) read as related. | Use alignment and subtle connectors to show hierarchy and relationship. |
| **Law of Prägnanz** | People interpret ambiguous things in the simplest way. | Prefer simple, clear shapes and layouts; avoid unnecessary visual complexity. |

### Memory & cognitive load

| Law | Idea | Apply by |
|-----|------|----------|
| **Miller's Law** | Working memory holds ~7±2 items. | Limit list lengths; chunk into groups of ~5–7; use recognition over recall. |
| **Chunking** | Break information into meaningful groups. | Group form fields, menu items, and list content into clear chunks. |
| **Cognitive Load** | Mental effort needed to use the interface. | Reduce steps, hide advanced options, use familiar patterns and clear labels. |

### Expectations & familiarity

| Law | Idea | Apply by |
|-----|------|----------|
| **Jakob's Law** | Users expect your product to behave like others they know. | Follow platform and web conventions (e.g. logo top-left, primary action prominent). |
| **Mental Model** | Users have a model of how the system works. | Match language and flow to the user's mental model; avoid surprising behavior. |
| **Paradox of the Active User** | Users start using without reading manuals. | Make the UI self-explanatory; use clear labels and in-context hints, not long docs. |

### Attention & emphasis

| Law | Idea | Apply by |
|-----|------|----------|
| **Von Restorff Effect** | The item that differs is remembered. | Use contrast (e.g. one primary button style) for the main action. |
| **Serial Position Effect** | First and last items in a list are remembered best. | Put critical items at start or end of lists/navigation. |
| **Peak-End Rule** | Experience is judged by peak and end moments. | Ensure key actions feel good and that completion/exit is clear and positive. |

### Simplicity & errors

| Law | Idea | Apply by |
|-----|------|----------|
| **Occam's Razor** | Prefer the solution with fewer assumptions. | Prefer simpler UI and fewer concepts when they achieve the same goal. |
| **Choice Overload** | Too many options cause overwhelm. | Limit options per screen; use filters and defaults; guide step-by-step. |
| **Aesthetic-Usability Effect** | Pleasing design is perceived as more usable. | Keep layout clean and consistent so the interface feels trustworthy and easy. |

## Review Checklist (by area)

**Buttons and actions**

- [ ] Primary action is larger or more prominent (Fitts's, Von Restorff).
- [ ] Number of choices per step is limited (Hick's, Choice Overload).
- [ ] Feedback or transition &lt;~400 ms or loading state shown (Doherty).

**Layout and hierarchy**

- [ ] Related items are close or in a clear region (Proximity, Common Region).
- [ ] Similar function uses similar appearance (Similarity).
- [ ] Structure is simple and unambiguous (Prägnanz, Occam's Razor).

**Navigation and lists**

- [ ] Menu/list items are chunked (Miller's, Chunking).
- [ ] Critical items at start or end if order matters (Serial Position).
- [ ] Matches platform conventions (Jakob's Law).

**Copy and learnability**

- [ ] Labels and flow match user expectations (Mental Model).
- [ ] Interface is understandable without a manual (Paradox of the Active User).

When suggesting changes, name the relevant law and how the current design violates or could better satisfy it.
