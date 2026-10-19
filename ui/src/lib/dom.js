/** Small DOM helpers shared by the hardware components. */

export const clamp = (value, min, max) => Math.min(max, Math.max(min, value));

/**
 * Create an element.
 *
 * Supported props: `class`, `text`, `html`, `dataset`, `style` (object),
 * `on<Event>` (function) and any DOM property or attribute.
 */
export function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (value === undefined || value === null || value === false) continue;
    if (key === "class") node.className = value;
    else if (key === "text") node.textContent = value;
    else if (key === "html") node.innerHTML = value;
    else if (key === "dataset") Object.assign(node.dataset, value);
    else if (key === "style" && typeof value === "object") Object.assign(node.style, value);
    else if (key.startsWith("on") && typeof value === "function") {
      node.addEventListener(key.slice(2).toLowerCase(), value);
    } else if (key in node) node[key] = value;
    else node.setAttribute(key, value === true ? "" : value);
  }
  append(node, children);
  return node;
}

function append(node, children) {
  for (const child of children.flat(Infinity)) {
    if (child === undefined || child === null || child === false) continue;
    node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
}

export function fillSelect(select, labels, value) {
  select.replaceChildren(...labels.map((label, index) => el("option", { value: index, text: label })));
  select.value = String(value ?? 0);
}

export function formatTime(ms) {
  const total = Math.max(0, ms) / 1000;
  const minutes = Math.floor(total / 60);
  const seconds = total - minutes * 60;
  return `${minutes}:${seconds.toFixed(1).padStart(4, "0")}`;
}

export function formatSigned(value, digits = 1) {
  const rounded = Number(value).toFixed(digits);
  return value > 0 ? `+${rounded}` : rounded;
}

/**
 * Rotary knob driven by vertical pointer drag, wheel and double-click reset.
 */
export function attachKnob(element, options = {}) {
  const { min = -1, max = 1, value = 0, step = 0.01, onChange, format } = options;
  let current = clamp(Number(value) || 0, min, max);
  let dragging = false;

  const paint = () => {
    const ratio = (current - min) / (max - min || 1);
    element.style.setProperty("--angle", `${-135 + ratio * 270}deg`);
    element.title = format ? format(current) : current.toFixed(2);
  };

  element.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    element.setPointerCapture(event.pointerId);
    dragging = true;
    const startY = event.clientY;
    const startValue = current;

    const move = (moveEvent) => {
      const delta = ((startY - moveEvent.clientY) / 140) * (max - min);
      current = clamp(startValue + delta, min, max);
      paint();
      onChange?.(current);
    };

    const finish = () => {
      dragging = false;
      element.removeEventListener("pointermove", move);
      element.removeEventListener("pointerup", finish);
      element.removeEventListener("pointercancel", finish);
    };

    element.addEventListener("pointermove", move);
    element.addEventListener("pointerup", finish);
    element.addEventListener("pointercancel", finish);
  });

  element.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const direction = event.deltaY > 0 ? -1 : 1;
      current = clamp(current + direction * step * (max - min), min, max);
      paint();
      onChange?.(current);
    },
    { passive: false },
  );

  element.addEventListener("dblclick", () => {
    current = min <= 0 && max >= 0 ? 0 : (min + max) / 2;
    paint();
    onChange?.(current);
  });

  paint();

  return {
    get value() {
      return current;
    },
    get dragging() {
      return dragging;
    },
    set(next) {
      if (dragging) return;
      current = clamp(Number(next) || 0, min, max);
      paint();
    },
    paint,
  };
}
