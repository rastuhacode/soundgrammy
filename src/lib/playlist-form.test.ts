import { describe, expect, it } from 'vitest'
import { fileBasename, validatePlaylistName } from './playlist-form'

describe('validatePlaylistName', () => {
  it('requires a non-empty trimmed name', () => {
    expect(validatePlaylistName('')).toBe('Playlist name is required')
    expect(validatePlaylistName('   ')).toBe('Playlist name is required')
  })

  it('rejects names longer than 100 characters', () => {
    expect(validatePlaylistName('a'.repeat(101))).toBe(
      'Playlist name must be at most 100 characters',
    )
  })

  it('accepts valid names', () => {
    expect(validatePlaylistName('Gym')).toBeNull()
    expect(validatePlaylistName('  Liked  ')).toBeNull()
    expect(validatePlaylistName('a'.repeat(100))).toBeNull()
  })
})

describe('fileBasename', () => {
  it('returns the last POSIX segment', () => {
    expect(fileBasename('/Users/me/Gym.soundgrammy.json')).toBe(
      'Gym.soundgrammy.json',
    )
  })

  it('returns the last Windows segment', () => {
    expect(fileBasename('C:\\Downloads\\Gym.soundgrammy.json')).toBe(
      'Gym.soundgrammy.json',
    )
  })

  it('returns the input when there is no separator', () => {
    expect(fileBasename('playlist.json')).toBe('playlist.json')
  })
})
