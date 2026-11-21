/**
 * The development host (dev.html): opens `public/sample/Sample.md`, or a file chosen in the page.
 * `?files=1` serves KitFiles too (new files beside the note under kit.json's `files` lists:
 * exports, pasted images, and printing), as a host with file handles does; without it, exports go
 * through the host's export write and pasted images through its create write.
 */
import { startDevHost } from '@kition/sdk/dev-host'
import manifest from '../../kit.json'

const files = new URLSearchParams(location.search).get('files') === '1'
startDevHost({ sample: './sample/Sample.md', ...(files ? { kitFiles: manifest.files } : {}) })
