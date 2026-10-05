import "./App.css";

import { useEffect } from "react";

import { SettingsPanel } from "./components/SettingsPanel";
import { Spotlight } from "./components/Spotlight";
import { TargetHighlight } from "./components/TargetHighlight";
import { getAppView } from "./services/desktop";

function App() {
  const view = getAppView();
  useEffect(() => {
    document.documentElement.dataset.view = view;
  }, [view]);
  if (view === "highlight") return <TargetHighlight />;
  return view === "settings" ? <SettingsPanel /> : <Spotlight />;
}

export default App;
