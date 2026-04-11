import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { afterEach, describe, expect, it, vi } from 'vitest'

const require = createRequire(import.meta.url)
const currentDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(currentDir, '../..')
const installDev = require('../../scripts/dev-cli/install-dev.cjs') as {
  run: (options: {
    env: Record<string, string>
    homeDir: string
    platform: NodeJS.Platform
    repoRoot: string
    existsSyncImpl: (path: string) => boolean
    readFileSyncImpl?: (path: string, encoding?: BufferEncoding) => string
    spawnSyncImpl: ReturnType<typeof vi.fn>
  }) => number
}

describe('install dev helper', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('runs cargo install directly when the first attempt succeeds', () => {
    const spawnSyncImpl = vi.fn(() => ({ status: 0, stdout: '', stderr: '' }))
    const existsSyncImpl = vi.fn(() => false)

    const exitCode = installDev.run({
      env: {},
      homeDir: 'C:/Users/dev',
      platform: 'win32',
      repoRoot,
      existsSyncImpl,
      spawnSyncImpl,
    })

    expect(exitCode).toBe(0)
    expect(spawnSyncImpl).toHaveBeenCalledTimes(1)
    expect(spawnSyncImpl).toHaveBeenCalledWith(
      'cargo',
      ['install', '--path', '.', '--force'],
      expect.objectContaining({
        cwd: repoRoot,
        encoding: 'utf8',
      }),
    )
  })

  it('stops the registered daemon before installing when the managed process matches', () => {
    const spawnSyncImpl = vi
      .fn()
      .mockReturnValueOnce({
        status: 0,
        stdout: JSON.stringify({
          pid: 4242,
          path: path.join('C:/Users/dev', '.cargo', 'bin', 'melo.exe'),
        }),
        stderr: '',
      })
      .mockReturnValueOnce({ status: 0, stdout: '', stderr: '' })
      .mockReturnValueOnce({ status: 1, stdout: '', stderr: '' })
      .mockReturnValueOnce({ status: 0, stdout: '', stderr: '' })
    const existsSyncImpl = vi.fn((value) => value === 'D:/state/daemon.json')
    const readFileSyncImpl = vi.fn(() =>
      JSON.stringify({
        pid: 4242,
      }),
    )

    const exitCode = installDev.run({
      env: {
        MELO_DAEMON_STATE_FILE: 'D:/state/daemon.json',
      },
      homeDir: 'C:/Users/dev',
      platform: 'win32',
      repoRoot,
      existsSyncImpl,
      readFileSyncImpl,
      spawnSyncImpl,
    })

    expect(exitCode).toBe(0)
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      1,
      'pwsh',
      expect.arrayContaining(['-NoLogo', '-Command']),
      expect.objectContaining({
        cwd: repoRoot,
        encoding: 'utf8',
      }),
    )
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      2,
      path.join('C:/Users/dev', '.cargo', 'bin', 'melo.exe'),
      ['daemon', 'stop'],
      expect.objectContaining({
        cwd: repoRoot,
        encoding: 'utf8',
      }),
    )
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      3,
      'pwsh',
      expect.arrayContaining(['-NoLogo', '-Command']),
      expect.objectContaining({
        cwd: repoRoot,
        encoding: 'utf8',
      }),
    )
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      4,
      'cargo',
      ['install', '--path', '.', '--force'],
      expect.objectContaining({
        cwd: repoRoot,
        encoding: 'utf8',
      }),
    )
  })

  it('skips daemon stop when the registered process path does not match the installed binary', () => {
    const spawnSyncImpl = vi
      .fn()
      .mockReturnValueOnce({
        status: 0,
        stdout: JSON.stringify({
          pid: 4242,
          path: 'D:/other/melo.exe',
        }),
        stderr: '',
      })
      .mockReturnValueOnce({ status: 0, stdout: '', stderr: '' })
    const existsSyncImpl = vi.fn((value) => value === 'D:/state/daemon.json')
    const readFileSyncImpl = vi.fn(() =>
      JSON.stringify({
        pid: 4242,
      }),
    )

    const exitCode = installDev.run({
      env: {
        MELO_DAEMON_STATE_FILE: 'D:/state/daemon.json',
      },
      homeDir: 'C:/Users/dev',
      platform: 'win32',
      repoRoot,
      existsSyncImpl,
      readFileSyncImpl,
      spawnSyncImpl,
    })

    expect(exitCode).toBe(0)
    expect(spawnSyncImpl).toHaveBeenCalledTimes(2)
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      2,
      'cargo',
      ['install', '--path', '.', '--force'],
      expect.objectContaining({
        cwd: repoRoot,
        encoding: 'utf8',
      }),
    )
  })
})
