import { useMemo, useState } from "react";

import { Icon } from "./Icon";
import type {
  LogicalRect,
  NormalizedUiElement,
  PixelRect,
  UiAutomationSnapshot,
  WindowContextSnapshot,
} from "../types/settings";

interface ContextInspectorProps {
  context: WindowContextSnapshot | null;
  capturing: boolean;
  highlightedElementId: string | null;
  highlightingElementId: string | null;
  inspecting: boolean;
  onCapture: () => void;
  onHighlight: (elementId: string) => void;
  onInspect: () => void;
  uiAutomation: UiAutomationSnapshot | null;
}

function formatPixelRect(rect: PixelRect): string {
  return `${rect.left}, ${rect.top} · ${rect.width} × ${rect.height} px`;
}

function formatLogicalRect(rect: LogicalRect): string {
  return `${rect.left.toFixed(1)}, ${rect.top.toFixed(1)} · ${rect.width.toFixed(1)} × ${rect.height.toFixed(1)} DIP`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function MetadataItem({ label, value, wide = false }: { label: string; value: string; wide?: boolean }) {
  return (
    <div className={wide ? "context-field context-field--wide" : "context-field"}>
      <dt>{label}</dt>
      <dd title={value}>{value || "Not reported"}</dd>
    </div>
  );
}

function elementSearchText(element: NormalizedUiElement): string {
  return [
    element.id,
    element.name,
    element.role,
    element.automation_id,
    element.class_name,
    element.framework_id,
    ...element.supported_patterns,
  ].join(" ").toLocaleLowerCase();
}

function UiTree({
  highlightedElementId,
  highlightingElementId,
  onHighlight,
  snapshot,
}: {
  highlightedElementId: string | null;
  highlightingElementId: string | null;
  onHighlight: (elementId: string) => void;
  snapshot: UiAutomationSnapshot;
}) {
  const [query, setQuery] = useState("");
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visibleElements = useMemo(
    () => normalizedQuery
      ? snapshot.elements.filter((element) => elementSearchText(element).includes(normalizedQuery))
      : snapshot.elements,
    [normalizedQuery, snapshot.elements],
  );

  return (
    <div className="uia-results" aria-live="polite">
      <div className="uia-summary">
        <div><strong>{snapshot.elements.length}</strong><span>included</span></div>
        <div><strong>{snapshot.visited_count}</strong><span>visited</span></div>
        <div><strong>{snapshot.filtered_count}</strong><span>filtered</span></div>
        <div><strong>{snapshot.duration_ms} ms</strong><span>duration</span></div>
      </div>

      <label className="uia-search">
        <span>Filter normalized elements</span>
        <input
          onChange={(event) => setQuery(event.currentTarget.value)}
          placeholder="Name, role, ID, class, or pattern"
          type="search"
          value={query}
        />
      </label>

      <div className="uia-tree-header">
        <span>{visibleElements.length} shown</span>
        <span>
          {highlightedElementId
            ? `${highlightedElementId} highlighted · select again to clear`
            : "Select a bounded element to highlight"}
        </span>
      </div>
      <ol className="uia-tree" aria-label="Normalized UI Automation tree">
        {visibleElements.map((element) => {
          const selected = highlightedElementId === element.id;
          const label = element.is_password ? "Protected field" : element.name || `${element.role} ${element.id}`;
          return (
            <li
              className={`${element.has_keyboard_focus ? "uia-node uia-node--focused" : "uia-node"}${selected ? " uia-node--highlighted" : ""}`}
              key={element.id}
            >
              <button
                aria-label={element.bounds_physical
                  ? `${selected ? "Clear highlight for" : "Highlight"} ${label}`
                  : `${label} has no screen bounds`}
                aria-pressed={selected}
                className="uia-node-button"
                disabled={!element.bounds_physical || (highlightingElementId !== null && highlightingElementId !== element.id)}
                onClick={() => onHighlight(element.id)}
                style={{ paddingLeft: `${12 + Math.min(element.depth, 12) * 18}px` }}
                type="button"
              >
                <div className="uia-node-main">
                  <code>{element.id}</code>
                  <span className="uia-role">{element.role}</span>
                  <strong title={element.name}>{element.is_password ? "Protected field" : element.name || "Unnamed"}</strong>
                  {highlightingElementId === element.id ? <em>Updating…</em> : null}
                </div>
                <div className="uia-node-detail">
                  {element.automation_id ? <span>ID {element.automation_id}</span> : null}
                  {element.class_name ? <span>Class {element.class_name}</span> : null}
                  {element.bounds_physical ? (
                    <span>{formatPixelRect(element.bounds_physical)}</span>
                  ) : <span>No bounds</span>}
                  {!element.is_enabled ? <span>Disabled</span> : null}
                  {element.is_keyboard_focusable ? <span>Focusable</span> : null}
                  {element.has_keyboard_focus ? <span>Focused</span> : null}
                </div>
                {element.supported_patterns.length > 0 ? (
                  <div className="uia-patterns">
                    {element.supported_patterns.map((pattern) => <span key={pattern}>{pattern}</span>)}
                  </div>
                ) : null}
              </button>
            </li>
          );
        })}
      </ol>
      {visibleElements.length === 0 ? (
        <p className="uia-no-results">No normalized elements match “{query.trim()}”.</p>
      ) : null}

      {snapshot.truncated || snapshot.warnings.length > 0 ? (
        <div className="context-warnings" role="status">
          {snapshot.truncated ? <p>The safety limits truncated this tree; the displayed elements remain usable.</p> : null}
          {snapshot.warnings.map((warning) => <p key={warning}>{warning}</p>)}
        </div>
      ) : null}
    </div>
  );
}

export function ContextInspector({
  context,
  capturing,
  highlightedElementId,
  highlightingElementId,
  inspecting,
  onCapture,
  onHighlight,
  onInspect,
  uiAutomation,
}: ContextInspectorProps) {
  return (
    <section className="context-inspector" aria-labelledby="context-heading">
      <div className="context-header">
        <div>
          <span className="eyebrow">Phase 2 diagnostics</span>
          <h2 id="context-heading">Windows context</h2>
          <p>
            Settings briefly hides so Windows can restore the target application. The capture stays
            in memory on this device and is never transmitted.
          </p>
        </div>
        <button
          aria-busy={capturing}
          className="button button--primary context-capture-button"
          disabled={capturing}
          onClick={onCapture}
          type="button"
        >
          <Icon name={context ? "refresh" : "camera"} size={17} />
          {capturing ? "Capturing…" : context ? "Capture again" : "Capture active application"}
        </button>
      </div>

      {context ? (
        <div className="context-results" aria-live="polite">
          <figure className="context-preview">
            <div className="context-preview-bar">
              <span>{context.process.name ?? "Unknown process"}</span>
              <span>{context.screenshot.width_px} × {context.screenshot.height_px}</span>
            </div>
            <img
              alt={`Captured active window: ${context.title || context.process.name || "untitled window"}`}
              draggable="false"
              src={context.screenshot.data_url}
            />
          </figure>

          <div className="context-section-heading">
            <div>
              <span className="context-status-dot" />
              <strong>Capture complete</strong>
            </div>
            <time dateTime={new Date(context.captured_at_unix_ms).toISOString()}>
              {new Date(context.captured_at_unix_ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
            </time>
          </div>

          <dl className="context-grid">
            <MetadataItem label="Window title" value={context.title} wide />
            <MetadataItem label="Process" value={context.process.name ?? "Access restricted"} />
            <MetadataItem label="Process ID" value={String(context.process.id)} />
            <MetadataItem label="Window class" value={context.class_name} />
            <MetadataItem label="Window handle" value={context.window_handle} />
            <MetadataItem label="Physical bounds" value={formatPixelRect(context.bounds_physical)} wide />
            <MetadataItem label="Logical bounds" value={formatLogicalRect(context.bounds_logical)} wide />
            <MetadataItem label="DPI and scale" value={`${context.dpi} DPI · ${context.scale_factor.toFixed(2)}×`} />
            <MetadataItem
              label="Monitor"
              value={`${context.monitor.device_name}${context.monitor.is_primary ? " · Primary" : ""}`}
            />
            <MetadataItem label="Monitor bounds" value={formatPixelRect(context.monitor.bounds_physical)} wide />
            <MetadataItem label="Monitor work area" value={formatPixelRect(context.monitor.work_area_physical)} wide />
            <MetadataItem
              label="Screenshot"
              value={`${context.screenshot.capture_method} · ${formatBytes(context.screenshot.byte_size)}`}
            />
            <MetadataItem
              label="Executable path"
              value={context.process.executable_path ?? "Windows restricted access"}
              wide
            />
          </dl>

          {context.warnings.length > 0 ? (
            <div className="context-warnings" role="status">
              {context.warnings.map((warning) => <p key={warning}>{warning}</p>)}
            </div>
          ) : null}
        </div>
      ) : (
        <div className="context-empty">
          <span><Icon name="camera" size={25} /></span>
          <h3>No context captured yet</h3>
          <p>Focus a normal desktop application, return here, and start a capture.</p>
        </div>
      )}

      <section className="uia-inspector" aria-labelledby="uia-heading">
        <div className="context-header uia-header">
          <div>
            <span className="eyebrow">Phase 3 developer inspector</span>
            <h2 id="uia-heading">Normalized UI tree</h2>
            <p>
              Reads the captured application’s accessible controls locally. This lists structure,
              states, bounds, and supported patterns. Select a bounded row to show a click-through
              highlight; no target action is invoked.
            </p>
          </div>
          <button
            aria-busy={inspecting}
            className="button button--secondary context-capture-button"
            disabled={!context || capturing || inspecting}
            onClick={onInspect}
            type="button"
          >
            <Icon name="sliders" size={17} />
            {inspecting ? "Inspecting…" : uiAutomation ? "Refresh UI tree" : "Inspect captured UI"}
          </button>
        </div>

        {uiAutomation ? (
          <UiTree
            highlightedElementId={highlightedElementId}
            highlightingElementId={highlightingElementId}
            onHighlight={onHighlight}
            snapshot={uiAutomation}
          />
        ) : (
          <div className="uia-empty">
            <strong>{context ? "Ready to inspect" : "Capture an application first"}</strong>
            <span>
              {context
                ? `Target: ${context.title || context.process.name || "untitled application"}`
                : "The UI tree is bound to the exact window captured above."}
            </span>
          </div>
        )}
      </section>
    </section>
  );
}
