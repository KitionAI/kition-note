import { describe, expect, it } from 'vitest'
import { keepRemoteImages } from './remoteImages'

/** A keeper that answers `attachments/pasted-<index>.png` for each address, recording the calls. */
function keeper(refuse: string[] = []) {
  const calls: Array<[string, number]> = []
  const keep = async (url: string, index: number) => {
    calls.push([url, index])
    if (refuse.includes(url)) throw new Error(`refused ${url}`)
    return `attachments/pasted-${index}.png`
  }
  return { calls, keep }
}

describe('keepRemoteImages', () => {
  it('keeps each pasted picture given by its web address once and links the kept file in its place', async () => {
    // HTML pasted from a document site: its pictures are signed addresses that expire in minutes.
    const remote = 'https://docs.example/uploader/x/i/Ab3dPictureId42x.jpg?g=Fx7docFileGuid9z&accessToken=eyJhbGciOiJIUzI1NiJ9.e30.sig'
    const pasted = [
      '- Writer',
      `  ![](${remote})`,
      '![chart](http://pictures.example/a.png "Q3")',
      `again ![](${remote})`,
      '![local](pictures/b.png) and [a page](https://example.com/page)',
    ].join('\n')
    const { calls, keep } = keeper()
    const { text, failed } = await keepRemoteImages(pasted, keep)
    expect(calls).toEqual([
      [remote, 1],
      ['http://pictures.example/a.png', 2],
    ])
    expect(text).toBe(
      [
        '- Writer',
        '  ![](attachments/pasted-1.png)',
        '![chart](attachments/pasted-2.png "Q3")',
        'again ![](attachments/pasted-1.png)',
        '![local](pictures/b.png) and [a page](https://example.com/page)',
      ].join('\n'),
    )
    expect(failed).toEqual([])
  })

  it('asks for an address as it is, without the escapes Markdown gives its parentheses', async () => {
    const { calls, keep } = keeper()
    const { text } = await keepRemoteImages('![](https://a.example/x\\(1\\).png)', keep)
    expect(calls).toEqual([['https://a.example/x(1).png', 1]])
    expect(text).toBe('![](attachments/pasted-1.png)')
  })

  it('leaves a picture it could not keep linked by its address and reports it', async () => {
    const { keep } = keeper(['https://a.example/gone.png'])
    const { text, failed } = await keepRemoteImages('![](https://a.example/gone.png)\n![](https://a.example/ok.png)', keep)
    expect(text).toBe('![](https://a.example/gone.png)\n![](attachments/pasted-2.png)')
    expect(failed.map(({ url }) => url)).toEqual(['https://a.example/gone.png'])
  })

  it('keeps nothing from text without pictures given by a web address', async () => {
    const { calls, keep } = keeper()
    const plain = 'text with ![a](a.png), ![b](data:image/png;base64,iVBO) and [c](https://c.example/c.png)'
    expect(await keepRemoteImages(plain, keep)).toEqual({ text: plain, failed: [] })
    expect(calls).toEqual([])
  })
})
