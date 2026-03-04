---
name: semantic-html
description: Use proper semantic HTML elements per MDN so structure reflects meaning for accessibility, SEO, and maintainability. Use when writing or reviewing HTML, Svelte, or other template markup.
---

# Semantic HTML

Use the right HTML element for the meaning of the content, not for how it looks. Presentation is the responsibility of CSS. Semantic markup improves accessibility (e.g. screen reader signposts), SEO, and makes code easier to understand and maintain.

**Source:** [MDN – Semantics in HTML](https://developer.mozilla.org/en-US/docs/Glossary/Semantics#semantics_in_html), [MDN – HTML elements reference](https://developer.mozilla.org/en-US/docs/Web/HTML/Element), [MDN – Document and website structure](https://developer.mozilla.org/en-US/docs/Learn/HTML/Introduction_to_HTML/Document_and_website_structure).

## When choosing markup

Ask: *"What element(s) best describe or represent this content?"* Examples: Is it a list (ordered or unordered)? An article with sections? Definitions? A figure with a caption? A page header or footer?

Use semantic elements first; use `<div>` or `<span>` only when no semantic element fits.

---

## Content sectioning (document structure)

Use these to create a clear outline and landmarks. Descriptions below follow MDN.

| Element | Use for |
|--------|--------|
| **`<header>`** | Introductory content for the page or a section: logo, nav aids, author name, etc. Can be the site-wide header (child of `<body>`) or the header of an `<article>` or `<section>`. |
| **`<nav>`** | A section whose purpose is navigation links (within the page or to other pages). Menus, tables of contents, indexes. Only main navigation; secondary links often go elsewhere. |
| **`<main>`** | The dominant content of the document: content that is directly related to or expands on the central topic or functionality. **Use once per page**, directly inside `<body>` when possible. |
| **`<aside>`** | Content only indirectly related to the main content: sidebars, call-outs, related links, glossary, author bio. Can be inside or outside `<main>`. |
| **`<footer>`** | Footer for the nearest sectioning root or sectioning content (e.g. `<body>`, `<article>`, `<section>`). Typically author info, copyright, related links. |
| **`<section>`** | A standalone thematic grouping that doesn’t have a more specific semantic element. Should almost always have a heading. Can contain `<article>`s; articles can contain `<section>`s. |
| **`<article>`** | Self-contained composition that could stand alone (e.g. syndication): forum post, blog entry, product card, comment, widget. |
| **`<search>`** | A part that contains form controls or other content for search or filtering. |
| **`<h1>`–`<h6>`** | Section headings. `<h1>` is the highest level; use a single `<h1>` per page. Don’t skip levels (e.g. `<h1>` then `<h4>`). |
| **`<hgroup>`** | Groups a heading with secondary content (subheading, tagline). |
| **`<address>`** | Contact information for a person, people, or organization. |

---

## Text content

Use these for blocks and structure of text; avoid using `<div>` when a semantic element applies.

| Element | Use for |
|--------|--------|
| **`<p>`** | Paragraphs. |
| **`<ul>`, `<ol>`, `<li>`** | Unordered and ordered lists; each item in `<li>`. Use `<ul>` for lists that aren’t step order or ranking. |
| **`<dl>`, `<dt>`, `<dd>`** | Description/definition lists: terms (`<dt>`) and descriptions (`<dd>`). Good for glossaries, key–value metadata. |
| **`<blockquote>`** | Extended quotation. Use `cite` for source URL; use `<cite>` for the source title. |
| **`<figure>`** | Self-contained content (image, diagram, code block) with optional caption. |
| **`<figcaption>`** | Caption or legend for the parent `<figure>`. |
| **`<hr>`** | Thematic break between paragraph-level content (e.g. change of scene or topic). |
| **`<pre>`** | Preformatted text, presented as written (e.g. code, ASCII art). |
| **`<div>`** | Generic flow container when no semantic element fits. No inherent meaning; use only when sectioning/text elements don’t apply. |

---

## Inline text semantics

Use for meaning of a word, phrase, or short span; don’t use for styling alone (use CSS for that).

| Element | Use for |
|--------|--------|
| **`<a>`** | Links (with `href`). |
| **`<strong>`** | Strong importance or urgency. |
| **`<em>`** | Stress emphasis (can be nested for degree). |
| **`<code>`** | Short fragment of code. |
| **`<kbd>`** | User input (keyboard, voice, etc.). |
| **`<samp>`** | Sample or quoted output from a program. |
| **`<var>`** | Variable in math or code. |
| **`<cite>`** | Title of a creative work (e.g. in a citation). |
| **`<q>`** | Short inline quotation (long quotes use `<blockquote>`). |
| **`<dfn>`** | The term being defined (in a definition phrase or `<dl>`). |
| **`<abbr>`** | Abbreviation or acronym; use `title` for expansion when helpful. |
| **`<time>`** | Date/time; use `datetime` for machine-readable value. |
| **`<mark>`** | Text highlighted for reference or notation. |
| **`<small>`** | Side comments, fine print (e.g. copyright), independent of font size. |
| **`<span>`** | Generic inline container when no other semantic element fits. Use only when necessary. |

Avoid using `<b>` and `<i>` for styling; use CSS. Use `<strong>` for importance and `<em>` for emphasis; use `<i>` only for idiomatic text, technical terms, etc., when appropriate per MDN.

---

## Rules of thumb

1. **One `<main>` per page** – Primary content only; place directly in `<body>` when possible.
2. **Heading hierarchy** – Single `<h1>`; don’t skip levels; structure reflects outline.
3. **Lists** – Use `<ul>`/`<ol>`/`<li>` for list content; use `<dl>`/`<dt>`/`<dd>` for terms and definitions.
4. **Prefer native semantics over ARIA** – Don’t add ARIA when an HTML element already conveys the role (e.g. use `<button>` not `<div role="button">`). See the fixing-accessibility skill when adding ARIA.
5. **Landmarks** – Use `<header>`, `<nav>`, `<main>`, `<aside>`, `<footer>` so assistive tech can jump to regions.
6. **Articles and sections** – Use `<article>` for self-contained content; use `<section>` for thematic grouping with a heading when no more specific element fits.
7. **Div and span** – Use only when no semantic block or inline element matches the content.

## References

- [MDN – Semantics in HTML](https://developer.mozilla.org/en-US/docs/Glossary/Semantics#semantics_in_html)
- [MDN – HTML elements reference (by function)](https://developer.mozilla.org/en-US/docs/Web/HTML/Element)
- [MDN – Document and website structure](https://developer.mozilla.org/en-US/docs/Learn/HTML/Introduction_to_HTML/Document_and_website_structure)
