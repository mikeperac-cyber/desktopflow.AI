import { Icon } from "./Icon";
import { hideWindow, type HideableAppView } from "../services/desktop";

interface WindowChromeProps {
  title: string;
  label: HideableAppView;
}

export function WindowChrome({ title, label }: WindowChromeProps) {
  return (
    <header className="window-chrome" data-tauri-drag-region>
      <span data-tauri-drag-region>{title}</span>
      <button
        aria-label={`Close ${title}`}
        className="icon-button"
        onClick={() => void hideWindow(label)}
        type="button"
      >
        <Icon name="close" size={18} />
      </button>
    </header>
  );
}
