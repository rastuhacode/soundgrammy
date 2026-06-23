import { create } from "zustand";
import type { PlaylistsBundle, Track } from "@/lib/db";
import { useLibraryStore } from "@/stores/library-store";
import { usePlayerStore } from "@/stores/player-store";

export const ALL_TRACKS_PLAYLIST_ID = "all" as const;
export type PlaylistId = typeof ALL_TRACKS_PLAYLIST_ID | number;

const SELECTED_PLAYLIST_STORAGE_KEY = "soundgrammy:selectedPlaylistId";

export type PlaylistsData = PlaylistsBundle;

export interface QueueSnapshot {
  playlistId: PlaylistId;
  trackIds: number[];
}

function readPersistedSelectedPlaylistId(): PlaylistId {
  if (typeof window === "undefined") {
    return ALL_TRACKS_PLAYLIST_ID;
  }

  const stored = localStorage.getItem(SELECTED_PLAYLIST_STORAGE_KEY);
  if (!stored || stored === ALL_TRACKS_PLAYLIST_ID) {
    return ALL_TRACKS_PLAYLIST_ID;
  }

  const parsed = Number(stored);
  return Number.isInteger(parsed) && parsed > 0
    ? parsed
    : ALL_TRACKS_PLAYLIST_ID;
}

function persistSelectedPlaylistId(id: PlaylistId) {
  if (typeof window === "undefined") {
    return;
  }
  localStorage.setItem(SELECTED_PLAYLIST_STORAGE_KEY, String(id));
}

function resolvePlaylistTrackIds(
  data: PlaylistsData,
  playlistId: PlaylistId,
): number[] {
  if (playlistId === ALL_TRACKS_PLAYLIST_ID) {
    return [];
  }

  if (playlistId === data.liked.id) {
    return data.liked.trackIds;
  }

  const custom = data.custom.find((playlist) => playlist.id === playlistId);
  return custom?.trackIds ?? [];
}

export function resolvePlaylistTracks(
  libraryTracks: Track[],
  data: PlaylistsData | null,
  playlistId: PlaylistId,
): Track[] {
  if (!data || playlistId === ALL_TRACKS_PLAYLIST_ID) {
    return libraryTracks;
  }

  const trackIds = resolvePlaylistTrackIds(data, playlistId);
  if (trackIds.length === 0) {
    return [];
  }

  const trackById = new Map(libraryTracks.map((track) => [track.id, track]));
  return trackIds
    .map((id) => trackById.get(id))
    .filter((track): track is Track => track !== undefined);
}

function isValidPlaylistId(
  data: PlaylistsData,
  playlistId: PlaylistId,
): boolean {
  if (playlistId === ALL_TRACKS_PLAYLIST_ID) {
    return true;
  }
  if (playlistId === data.liked.id) {
    return true;
  }
  return data.custom.some((playlist) => playlist.id === playlistId);
}

function createQueueSnapshot(
  libraryTracks: Track[],
  data: PlaylistsData | null,
  playlistId: PlaylistId,
): QueueSnapshot {
  const trackIds = resolvePlaylistTracks(libraryTracks, data, playlistId).map(
    (track) => track.id,
  );
  return { playlistId, trackIds };
}

interface PlaylistsState {
  data: PlaylistsData | null;
  selectedPlaylistId: PlaylistId;
  activePlaylistId: PlaylistId;
  queueSnapshot: QueueSnapshot | null;
  hydrate: (data: PlaylistsData) => void;
  setSelectedPlaylist: (id: PlaylistId) => void;
  activateSelectedPlaylist: (trackId: number) => void;
  setData: (data: PlaylistsData) => void;
  syncQueueToPlayer: () => void;
}

export const usePlaylistsStore = create<PlaylistsState>((set, get) => ({
  data: null,
  selectedPlaylistId: ALL_TRACKS_PLAYLIST_ID,
  activePlaylistId: ALL_TRACKS_PLAYLIST_ID,
  queueSnapshot: null,

  hydrate: (data) => {
    const persisted = readPersistedSelectedPlaylistId();
    const selectedPlaylistId = isValidPlaylistId(data, persisted)
      ? persisted
      : ALL_TRACKS_PLAYLIST_ID;

    if (selectedPlaylistId !== persisted) {
      persistSelectedPlaylistId(selectedPlaylistId);
    }

    set({
      data,
      selectedPlaylistId,
      activePlaylistId: selectedPlaylistId,
      queueSnapshot: null,
    });
  },

  setSelectedPlaylist: (id) => {
    persistSelectedPlaylistId(id);
    set({ selectedPlaylistId: id });
  },

  activateSelectedPlaylist: (trackId) => {
    const { selectedPlaylistId, queueSnapshot, data } = get();
    const libraryTracks = useLibraryStore.getState().tracks;
    const currentTrackId = usePlayerStore.getState().currentTrack?.id ?? null;

    const needsNewSnapshot
      = !queueSnapshot
        || queueSnapshot.playlistId !== selectedPlaylistId
        || currentTrackId !== trackId;

    if (needsNewSnapshot) {
      set({
        activePlaylistId: selectedPlaylistId,
        queueSnapshot: createQueueSnapshot(
          libraryTracks,
          data,
          selectedPlaylistId,
        ),
      });
    } else {
      set({ activePlaylistId: selectedPlaylistId });
    }

    get().syncQueueToPlayer();
  },

  setData: (data) => {
    const { selectedPlaylistId, activePlaylistId, queueSnapshot } = get();
    const nextSelectedPlaylistId = isValidPlaylistId(data, selectedPlaylistId)
      ? selectedPlaylistId
      : ALL_TRACKS_PLAYLIST_ID;
    const nextActivePlaylistId = isValidPlaylistId(data, activePlaylistId)
      ? activePlaylistId
      : ALL_TRACKS_PLAYLIST_ID;

    let nextQueueSnapshot = queueSnapshot;
    if (
      nextQueueSnapshot
      && !isValidPlaylistId(data, nextQueueSnapshot.playlistId)
    ) {
      nextQueueSnapshot = null;
    }

    if (nextSelectedPlaylistId !== selectedPlaylistId) {
      persistSelectedPlaylistId(nextSelectedPlaylistId);
    }

    set({
      data,
      selectedPlaylistId: nextSelectedPlaylistId,
      activePlaylistId: nextActivePlaylistId,
      queueSnapshot: nextQueueSnapshot,
    });

    get().syncQueueToPlayer();
  },

  syncQueueToPlayer: () => {
    const { queueSnapshot } = get();
    if (!queueSnapshot) return;

    const libraryTracks = useLibraryStore.getState().tracks;
    const trackById = new Map(libraryTracks.map((track) => [track.id, track]));
    const validTrackIds = queueSnapshot.trackIds.filter((id) =>
      trackById.has(id),
    );

    if (validTrackIds.length !== queueSnapshot.trackIds.length) {
      set({
        queueSnapshot: {
          ...queueSnapshot,
          trackIds: validTrackIds,
        },
      });
    }

    const queueTracks = validTrackIds
      .map((id) => trackById.get(id))
      .filter((track): track is Track => track !== undefined);

    usePlayerStore.getState().setQueueTracks(queueTracks);
  },
}));

export function getLikedTrackIdSet(data: PlaylistsData | null): Set<number> {
  if (!data) {
    return new Set();
  }
  return new Set(data.liked.trackIds);
}

export function isTrackLiked(
  data: PlaylistsData | null,
  trackId: number,
): boolean {
  return getLikedTrackIdSet(data).has(trackId);
}
