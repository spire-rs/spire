// One synchronous capture: attributes and observations refer to the same DOM.
const selector = arguments[0];
let selected;
try {
    selected = Array.from(document.querySelectorAll('a[href], area[href]'))
        .filter(link => selector === null || link.matches(selector));
} catch (error) {
    return { error: String(error) };
}

function intersect(a, b, x = true, y = true) {
    return {
        left: x ? Math.max(a.left, b.left) : a.left,
        right: x ? Math.min(a.right, b.right) : a.right,
        top: y ? Math.max(a.top, b.top) : a.top,
        bottom: y ? Math.min(a.bottom, b.bottom) : a.bottom,
    };
}
function hasArea(rect) { return rect.right > rect.left && rect.bottom > rect.top; }
function clipRect(style, box) {
    if (style.clip !== 'auto') {
        const values = style.clip.match(/^rect\((.+)\)$/)?.[1].split(/[,\s]+/)
            .filter(Boolean).map(value => Number.parseFloat(value));
        if (values?.length === 4 && values.every(Number.isFinite)) {
            box = intersect(box, {
                top: box.top + values[0], right: box.left + values[1],
                bottom: box.top + values[2], left: box.left + values[3],
            });
        } else { return null; }
    }
    if (style.clipPath !== 'none') {
        const inset = style.clipPath.match(/^inset\(([^()]+)\)$/)?.[1];
        if (!inset || inset.includes('round')) { return null; }
        const parts = inset.trim().split(/\s+/);
        if (parts.length > 4) { return null; }
        const [top, right = top, bottom = top, left = right] = parts;
        const length = (value, size) => value.endsWith('%')
            ? Number.parseFloat(value) * size / 100 : Number.parseFloat(value);
        const width = box.right - box.left, height = box.bottom - box.top;
        const values = [length(top, height), length(right, width), length(bottom, height), length(left, width)];
        if (!values.every(Number.isFinite)) { return null; }
        box = intersect(box, {
            top: box.top + values[0], right: box.right - values[1],
            bottom: box.bottom - values[2], left: box.left + values[3],
        });
    }
    return box;
}
function visibility(element) {
    const ownStyle = getComputedStyle(element);
    if (ownStyle.visibility !== 'visible') {
        // Visibility can be restored on a descendant. Do not discard such
        // a link just because the anchor's own inherited value is hidden.
        const restored = Array.from(element.querySelectorAll('*')).some(child =>
            getComputedStyle(child).visibility === 'visible'
            && Array.from(child.getClientRects()).some(hasArea));
        return restored ? 'unknown' : 'hidden';
    }
    const ancestors = [];
    for (let node = element; node; node = node.parentElement) {
        const style = getComputedStyle(node);
        if (node.hasAttribute('hidden') || style.display === 'none'
            || style.contentVisibility === 'hidden' || Number(style.opacity) === 0
            || (node.getAttribute('aria-hidden')?.toLowerCase() === 'true'
                && node.getAttribute('tabindex')?.trim() === '-1')) { return 'hidden'; }
        ancestors.push([node, style]);
    }
    // A display:contents anchor or an anchor wrapping an image can have no
    // box of its own, while its descendants still offer a clickable area.
    const boxes = Array.from(element.getClientRects());
    if (!boxes.some(hasArea)) {
        for (const child of element.childNodes) {
            if (child.nodeType === Node.TEXT_NODE && child.textContent.trim()) {
                const range = document.createRange();
                range.selectNode(child);
                boxes.push(...range.getClientRects());
            }
        }
        for (const child of element.querySelectorAll('*')) {
            if (getComputedStyle(child).visibility === 'visible') {
                boxes.push(...child.getClientRects());
            }
        }
    }
    let uncertain = false;
    for (const original of boxes) {
        if (!hasArea(original)) { continue; }
        let box = original, unknown = false;
        for (const [node, style] of ancestors) {
            const bounds = node.getBoundingClientRect();
            const clipX = ['hidden', 'clip', 'scroll', 'auto'].includes(style.overflowX);
            const clipY = ['hidden', 'clip', 'scroll', 'auto'].includes(style.overflowY);
            // Scroll containers can reveal content; only non-scrollable
            // clipping is evidence of hidden content.
            box = intersect(box, bounds,
                clipX && ['hidden', 'clip'].includes(style.overflowX),
                clipY && ['hidden', 'clip'].includes(style.overflowY));
            const clipping = clipRect(style, bounds);
            if (clipping === null) { unknown = true; }
            else if (style.clip !== 'auto' || style.clipPath !== 'none') {
                box = intersect(box, clipping);
            }
            if (style.position === 'fixed') {
                box = intersect(box, { left: 0, top: 0, right: innerWidth, bottom: innerHeight });
            }
        }
        // Do not intersect ordinary content with the viewport: below the
        // fold remains followable. Negative document coordinates are not.
        box = intersect(box, { left: -scrollX, top: -scrollY, right: Infinity, bottom: Infinity });
        if (hasArea(box)) {
            if (!unknown) { return 'visible'; }
            uncertain = true;
        }
    }
    return uncertain ? 'unknown' : 'hidden';
}
function linkVisibility(link) {
    if (link.localName !== 'area') { return visibility(link); }
    for (let node = link; node; node = node.parentElement) {
        if (node.hasAttribute('hidden') ||
            (node.getAttribute('aria-hidden')?.toLowerCase() === 'true'
                && node.getAttribute('tabindex')?.trim() === '-1')) { return 'hidden'; }
    }
    // Areas have no layout box. Inspect their associated image instead;
    // precise shape hit-testing is unknown rather than inferred from size.
    const map = link.closest('map[name]');
    if (!map) { return 'unknown'; }
    const images = Array.from(document.images).filter(image => image.getAttribute('usemap') === '#' + map.name);
    if (images.length === 0) { return 'unknown'; }
    return images.every(image => visibility(image) === 'hidden') ? 'hidden' : 'unknown';
}
return {
    uri: document.URL,
    base_uri: document.baseURI,
    links: selected.map(link => ({
        href: link.getAttribute('href'),
        rel: Array.from(link.relList),
        text: link.localName === 'area' ? link.alt : link.textContent,
        kind: link.localName,
        visibility: linkVisibility(link),
    })),
};
