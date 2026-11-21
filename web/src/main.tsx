import { createRoot } from 'react-dom/client'
import { connectHost, createKitStrings, defineKitUi } from '@kition/sdk/editor'
import '@kition/sdk/editor/styles.css'
import './app.css'
import { App } from './App'
import { NOTE_MENUS } from './menus'
import { CATALOGS } from './strings'
import manifest from '../../kit.json'
import './commandIcons'

/** Every element this editor draws besides engine commands: its menus, panels, and controls (Kit UI protocol section 9). */
const KIT_UI = defineKitUi({
  menus: NOTE_MENUS,
  tools: [],
  panels: [
    { id: 'panel:outline', label: 'outline' },
    { id: 'panel:comments', label: 'comments' },
  ],
  tabs: [],
  groups: [],
  controls: [],
})

const root = createRoot(document.getElementById('root')!)

// Opened outside a host, the page says where it belongs instead of waiting forever.
const alone = window.setTimeout(() => {
  const t = createKitStrings(CATALOGS, navigator.language)
  root.render(<div className="kui-alone">{t('noHost')}</div>)
}, 3000)

// `replica`: the host keeps the note's replica with its file, so comments (which never go into the
// `.md`), version history, and co-editing follow the note (Kit UI protocol section 7); other
// people's changes come in through `replica/import` and show at once. `presence`: the person's
// caret and selection are shared, and the others' drawn (section 7e).
// `comment-anchors`: with a host that keeps comment threads itself (section 7k), a new thread on the
// selection is the host's (`comment.create`) and the note marks the threads the host shows.
// `brand` is Kition's purple (`ui.brand`); the editor opens in the single-line display mode (`ui.display`).
void connectHost({ id: 'kit.note', version: manifest.version, capabilities: ['ai', 'replica', 'presence', 'comment-anchors'], ui: KIT_UI, brand: manifest.ui.brand, display: manifest.ui.display }).then((host) => {
  window.clearTimeout(alone)
  // No StrictMode: its doubled effects would open the note twice, in two engine workers.
  root.render(<App host={host} />)
})
