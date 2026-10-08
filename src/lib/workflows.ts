export type WorkflowCategory = "Files" | "Text" | "System" | "Productivity";

export interface BuiltInWorkflow {
  id: string;
  name: string;
  description: string;
  category: WorkflowCategory;
  app_hint: string;
  instruction: string;
}

export const WORKFLOW_CATEGORIES: WorkflowCategory[] = ["Files", "Text", "System", "Productivity"];

export const MAX_SAVED_WORKFLOWS = 50;
export const MAX_WORKFLOW_NAME_CHARS = 80;
export const MAX_WORKFLOW_INSTRUCTION_CHARS = 4000;

export function makeUserWorkflowId(): string {
  return `user-${Date.now().toString(36)}-${Math.floor(Math.random() * 0xffff).toString(36)}`;
}

// Every recipe is a bounded instruction for the validated planning pipeline:
// concrete controls, explicit values, no secrets, nothing destructive. Each
// one still produces a plan that requires the unchanged second confirmation.
export const BUILT_IN_WORKFLOWS: BuiltInWorkflow[] = [
  {
    id: "notepad-save-as-desktop",
    name: "Save document to Desktop",
    description: "Opens Save As and navigates to the Desktop, leaving the name for you.",
    category: "Text",
    app_hint: "Open the file in Notepad first",
    instruction:
      "Open the File menu, choose Save As, navigate to the Desktop folder, and stop with the dialog open. Do not type a file name or press Save.",
  },
  {
    id: "notepad-find-replace",
    name: "Find and replace a word",
    description: "Replaces every TODO with DONE in the open document.",
    category: "Text",
    app_hint: "Open the document in Notepad first",
    instruction:
      "Open the Edit menu, choose Replace, type TODO in the Find what field and DONE in the Replace with field, then click Replace All exactly once.",
  },
  {
    id: "notepad-zoom",
    name: "Zoom text to 150%",
    description: "Enlarges the editor text via the View zoom controls.",
    category: "Text",
    app_hint: "Open the document in Notepad first",
    instruction:
      "Open the View menu, choose Zoom, then choose Zoom In repeatedly until the status bar reports 150%.",
  },
  {
    id: "notepad-word-count",
    name: "Show word count",
    description: "Opens the word-count dialog for the current document.",
    category: "Text",
    app_hint: "Open the document in Notepad first",
    instruction:
      "Open the View menu and choose Word Count, leaving the count dialog open for review.",
  },
  {
    id: "explorer-new-folder",
    name: "Create a project folder",
    description: "Creates and opens a folder named DeskFlow Inbox here.",
    category: "Files",
    app_hint: "Open the destination in File Explorer first",
    instruction:
      "Create a new folder named 'DeskFlow Inbox' in the current location, then open it.",
  },
  {
    id: "explorer-rename-selected",
    name: "Rename the selected file",
    description: "Renames the highlighted file to meeting-notes.txt.",
    category: "Files",
    app_hint: "Select the file in File Explorer first",
    instruction:
      "Rename the currently selected file to 'meeting-notes.txt' and confirm with Enter.",
  },
  {
    id: "explorer-sort-downloads",
    name: "Sort Downloads by date",
    description: "Shows the newest downloads first.",
    category: "Files",
    app_hint: "Open the Downloads folder first",
    instruction:
      "Open the Sort options in the View menu and sort by Date modified, newest first.",
  },
  {
    id: "explorer-show-extensions",
    name: "Show file extensions",
    description: "Unhides extensions for known file types.",
    category: "Files",
    app_hint: "Open any folder in File Explorer first",
    instruction:
      "Open the View menu, choose Options, switch to the View tab, and uncheck 'Hide extensions for known file types', then apply.",
  },
  {
    id: "explorer-details-view",
    name: "Switch to Details view",
    description: "Changes the current folder to the Details layout.",
    category: "Files",
    app_hint: "Open the folder in File Explorer first",
    instruction: "Open the View menu and switch the layout to Details.",
  },
  {
    id: "paint-resize-canvas",
    name: "Resize canvas to 800 by 600",
    description: "Sets exact pixel dimensions without keeping aspect ratio.",
    category: "Productivity",
    app_hint: "Open the image in Paint first",
    instruction:
      "Open the Resize option, choose Pixels, uncheck Maintain aspect ratio, set 800 by 600, and apply.",
  },
  {
    id: "paint-save-png",
    name: "Save image as PNG",
    description: "Saves a copy named deskflow-sketch into Pictures.",
    category: "Productivity",
    app_hint: "Open the image in Paint first",
    instruction:
      "Open File, choose Save As, then PNG picture. Navigate to the Pictures folder, name the file 'deskflow-sketch', and save.",
  },
  {
    id: "calculator-tax",
    name: "Add 8 percent tax",
    description: "Computes 1,250 plus 8% using on-screen buttons.",
    category: "Productivity",
    app_hint: "Open Calculator first",
    instruction:
      "Switch to Standard mode if needed, then compute 1250 plus 8 percent using the on-screen buttons, leaving the result displayed.",
  },
  {
    id: "sticky-new-note",
    name: "Start a shopping note",
    description: "Creates a note prefilled with a short list.",
    category: "Productivity",
    app_hint: "Open Sticky Notes first",
    instruction: "Create a new note and type 'Shopping: milk, eggs, bread' as its content.",
  },
  {
    id: "clock-focus-timer",
    name: "Start a 25-minute timer",
    description: "Adds and starts a timer named Focus.",
    category: "Productivity",
    app_hint: "Open the Clock app first",
    instruction:
      "Open the Timer section, add a new 25 minute timer named 'Focus', and start it.",
  },
  {
    id: "settings-dark-mode",
    name: "Turn on dark mode",
    description: "Switches Windows personalization to Dark.",
    category: "System",
    app_hint: "Open Windows Settings first",
    instruction: "Open Personalization, then Colors, and set Choose your mode to Dark.",
  },
  {
    id: "settings-night-light",
    name: "Turn on Night light",
    description: "Enables the warmer display mode.",
    category: "System",
    app_hint: "Open Windows Settings first",
    instruction: "Open System, then Display, and turn Night light on.",
  },
  {
    id: "taskmgr-sort-memory",
    name: "Sort processes by memory",
    description: "Surfaces the heaviest processes first.",
    category: "System",
    app_hint: "Open Task Manager first",
    instruction: "Open the Processes tab and sort by Memory, highest first. Do not end any task.",
  },
  {
    id: "winver",
    name: "Show Windows version",
    description: "Opens the Run dialog and displays winver.",
    category: "System",
    app_hint: "Works from anywhere",
    instruction:
      "Open the Run dialog with Win+R, type 'winver', press Enter, and leave the version window open.",
  },
  {
    id: "edge-bookmark",
    name: "Bookmark this page",
    description: "Saves the current tab to bookmarks.",
    category: "Productivity",
    app_hint: "Open the page in Edge first",
    instruction: "Press Ctrl+D to bookmark the current page and confirm the bookmark dialog.",
  },
  {
    id: "edge-private-window",
    name: "Open a private window",
    description: "Opens a new InPrivate browsing window.",
    category: "Productivity",
    app_hint: "Open Edge first",
    instruction: "Open the browser menu and choose New InPrivate window.",
  },
];
