import { useEffect, useState } from "react";

import { addMemory, deleteMemory, loadMemories, purgeMemories, toUserMessage } from "../services/desktop";
import type { MemoryEntry } from "../types/settings";

export function MemoryManager() {
  const [memories, setMemories] = useState<MemoryEntry[]>([]);
  const [subject, setSubject] = useState("");
  const [content, setContent] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void loadMemories().then(setMemories).catch((loadError: unknown) => setError(toUserMessage(loadError)));
  }, []);

  const canAdd =
    content.trim().length > 0 &&
    content.trim().length <= 500 &&
    subject.trim().length <= 64 &&
    memories.length < 100 &&
    !busy;

  async function handleAdd() {
    if (!canAdd) return;
    setBusy(true);
    setError(null);
    try {
      setMemories(await addMemory(subject.trim(), content.trim()));
      setSubject("");
      setContent("");
    } catch (addError: unknown) {
      setError(toUserMessage(addError));
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete(id: string) {
    setBusy(true);
    setError(null);
    try {
      setMemories(await deleteMemory(id));
    } catch (deleteError: unknown) {
      setError(toUserMessage(deleteError));
    } finally {
      setBusy(false);
    }
  }

  async function handlePurge() {
    setBusy(true);
    setError(null);
    try {
      setMemories(await purgeMemories());
    } catch (purgeError: unknown) {
      setError(toUserMessage(purgeError));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-group" aria-labelledby="local-memory-heading">
      <h2 id="local-memory-heading">Local memory</h2>
      <p className="ai-provider-copy">
        Your own long-lived notes live in a file on this PC and are added to planning prompts for
        matching apps. Memories travel to the selected provider with the plan request — hosted
        providers receive the text. Never store secrets here; purge everything any time.
      </p>

      <ul className="workflow-list">
        {memories.map((memory) => (
          <li className="workflow-card" key={memory.id}>
            <div>
              <strong>{memory.subject ? `[${memory.subject}] ` : ""}{memory.content}</strong>
              <small>Saved {new Date(memory.created_at_unix_ms).toLocaleDateString()}</small>
            </div>
            <div className="workflow-actions">
              <button
                className="button button--secondary"
                disabled={busy}
                onClick={() => void handleDelete(memory.id)}
                type="button"
              >
                Forget
              </button>
            </div>
          </li>
        ))}
      </ul>
      {memories.length === 0 ? (
        <p className="provider-setup-note">No memories yet. DeskFlow plans from fresh observation alone.</p>
      ) : null}

      <div className="setting-row setting-row--input provider-picker-row">
        <div>
          <label htmlFor="memory-subject">App (optional)</label>
          <p>Blank applies everywhere, e.g. notepad.exe for one app.</p>
        </div>
        <input
          id="memory-subject"
          maxLength={64}
          onChange={(event) => setSubject(event.currentTarget.value)}
          placeholder="notepad.exe"
          spellCheck={false}
          type="text"
          value={subject}
        />
      </div>
      <label htmlFor="memory-content">Memory</label>
      <textarea
        id="memory-content"
        maxLength={500}
        onChange={(event) => setContent(event.currentTarget.value)}
        placeholder="Keep word wrap on and confirm before closing windows…"
        value={content}
      />
      <div className="ai-plan-actions">
        <span>{content.trim().length.toLocaleString()} / 500</span>
        <button className="button button--secondary" disabled={!canAdd} onClick={() => void handleAdd()} type="button">
          {busy ? "Saving…" : "Remember this"}
        </button>
      </div>

      {memories.length > 0 ? (
        <button className="button button--secondary" disabled={busy} onClick={() => void handlePurge()} type="button">
          Forget everything
        </button>
      ) : null}
      {error ? <div className="inline-error" role="alert">{error}</div> : null}
    </section>
  );
}
