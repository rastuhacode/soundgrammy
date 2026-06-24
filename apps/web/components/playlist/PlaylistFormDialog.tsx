"use client";

import { useEffect, useId, useRef, useState } from "react";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { ImagePlus, Trash2 } from "lucide-react";
import { z } from "zod";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import type { CustomPlaylistSummary } from "@/lib/db";
import {
  createThumbnailPreviewUrl,
  playlistThumbnailUrl,
  readPlaylistThumbnailFile,
  revokeThumbnailPreviewUrl,
} from "@/lib/playlist-thumbnail";
import { usePlaylistsStore } from "@/stores/playlists-store";
import { useTRPC } from "@/trpc/client";

const playlistFormSchema = z.object({
  name: z
    .string()
    .trim()
    .min(1, "Playlist name is required")
    .max(100, "Playlist name must be at most 100 characters"),
});

type PlaylistFormMode = "create" | "edit";

interface PlaylistFormDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mode: PlaylistFormMode;
  playlist?: CustomPlaylistSummary;
}

export function PlaylistFormDialog({
  open,
  onOpenChange,
  mode,
  playlist,
}: PlaylistFormDialogProps) {
  const formId = useId();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const trpc = useTRPC();
  const data = usePlaylistsStore((state) => state.data);
  const setData = usePlaylistsStore((state) => state.setData);
  const setSelectedPlaylist = usePlaylistsStore(
    (state) => state.setSelectedPlaylist,
  );

  const [thumbnailFile, setThumbnailFile] = useState<File | null>(null);
  const [thumbnailPreview, setThumbnailPreview] = useState<string | null>(null);
  const [removeThumbnail, setRemoveThumbnail] = useState(false);
  const [thumbnailError, setThumbnailError] = useState<string | null>(null);

  const createMutation = useMutation(trpc.playlists.create.mutationOptions());
  const updateMutation = useMutation(trpc.playlists.update.mutationOptions());

  const isSubmitting = createMutation.isPending || updateMutation.isPending;
  const isEdit = mode === "edit";

  const form = useForm({
    defaultValues: {
      name: playlist?.name ?? "",
    },
    validators: {
      onSubmit: playlistFormSchema,
    },
    onSubmit: async ({ value }) => {
      if (!data) return;

      setThumbnailError(null);

      let thumbnailPayload:
        | { data: string; mime: "image/jpeg" | "image/png" | "image/webp" }
        | null
        | undefined;

      try {
        if (thumbnailFile) {
          thumbnailPayload = await readPlaylistThumbnailFile(thumbnailFile);
        } else if (removeThumbnail) {
          thumbnailPayload = null;
        }
      } catch (error) {
        setThumbnailError(
          error instanceof Error ? error.message : "Invalid thumbnail",
        );
        return;
      }

      try {
        if (isEdit && playlist) {
          const updated = await updateMutation.mutateAsync({
            id: playlist.id,
            name: value.name,
            thumbnail: thumbnailPayload,
          });

          setData({
            ...data,
            custom: data.custom.map((item) =>
              item.id === updated.id ? updated : item,
            ),
          });
        } else {
          const created = await createMutation.mutateAsync({
            name: value.name,
            thumbnail: thumbnailPayload ?? undefined,
          });

          setData({
            ...data,
            custom: [...data.custom, created],
          });
          setSelectedPlaylist(created.id);
        }

        onOpenChange(false);
      } catch {
        // mutation error surfaced by react-query if needed
      }
    },
  });

  useEffect(() => {
    if (!open) return;

    form.setFieldValue("name", playlist?.name ?? "");
    setThumbnailFile(null);
    setRemoveThumbnail(false);
    setThumbnailError(null);

    if (playlist?.hasThumbnail) {
      setThumbnailPreview(playlistThumbnailUrl(playlist.id));
    } else {
      setThumbnailPreview(null);
    }
  }, [open, playlist, form]);

  useEffect(() => {
    return () => {
      revokeThumbnailPreviewUrl(thumbnailPreview);
    };
  }, [thumbnailPreview]);

  const handleThumbnailChange = (file: File | null) => {
    setThumbnailError(null);
    setRemoveThumbnail(false);

    if (!file) {
      setThumbnailFile(null);
      if (isEdit && playlist?.hasThumbnail) {
        setThumbnailPreview(playlistThumbnailUrl(playlist.id));
      } else {
        revokeThumbnailPreviewUrl(thumbnailPreview);
        setThumbnailPreview(null);
      }
      return;
    }

    revokeThumbnailPreviewUrl(thumbnailPreview);
    setThumbnailFile(file);
    setThumbnailPreview(createThumbnailPreviewUrl(file));
  };

  const handleRemoveThumbnail = () => {
    setThumbnailError(null);
    setThumbnailFile(null);
    setRemoveThumbnail(true);
    revokeThumbnailPreviewUrl(thumbnailPreview);
    setThumbnailPreview(null);
    if (fileInputRef.current) {
      fileInputRef.current.value = "";
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {isEdit ? "Edit playlist" : "Create playlist"}
          </DialogTitle>
          <DialogDescription>
            {isEdit
              ? "Update the playlist name or cover image."
              : "Give your playlist a name and optional cover image."}
          </DialogDescription>
        </DialogHeader>

        <form
          id={formId}
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          <FieldGroup>
            <form.Field
              name="name"
              children={(field) => {
                const isInvalid
                  = field.state.meta.isTouched && !field.state.meta.isValid;

                return (
                  <Field data-invalid={isInvalid}>
                    <FieldLabel htmlFor={`${formId}-name`}>Name</FieldLabel>
                    <Input
                      id={`${formId}-name`}
                      name={field.name}
                      value={field.state.value}
                      onBlur={field.handleBlur}
                      onChange={(event) =>
                        field.handleChange(event.target.value)}
                      aria-invalid={isInvalid}
                      placeholder="My playlist"
                      autoComplete="off"
                    />
                    {isInvalid
                      ? <FieldError errors={field.state.meta.errors} />
                      : null}
                  </Field>
                );
              }}
            />

            <Field>
              <FieldLabel htmlFor={`${formId}-thumbnail`}>Cover image</FieldLabel>
              <div className="flex items-start gap-4">
                <div className="relative size-24 shrink-0 overflow-hidden rounded-lg bg-muted shadow-sm ring-1 ring-border/60">
                  {thumbnailPreview
                    ? (
                        <img
                          src={thumbnailPreview}
                          alt="Playlist cover preview"
                          className="size-full object-cover"
                        />
                      )
                    : (
                        <div className="flex size-full items-center justify-center bg-linear-to-br from-slate-600 to-slate-800 text-white/80">
                          <ImagePlus className="size-8" />
                        </div>
                      )}
                </div>

                <div className="flex min-w-0 flex-1 flex-col gap-2">
                  <Input
                    ref={fileInputRef}
                    id={`${formId}-thumbnail`}
                    type="file"
                    accept="image/jpeg,image/png,image/webp"
                    onChange={(event) =>
                      handleThumbnailChange(event.target.files?.[0] ?? null)}
                  />
                  <FieldDescription>
                    JPEG, PNG, or WebP up to 512KB.
                  </FieldDescription>
                  {(thumbnailPreview || playlist?.hasThumbnail) && !removeThumbnail
                    ? (
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={handleRemoveThumbnail}
                          className="w-fit"
                        >
                          <Trash2 />
                          Remove image
                        </Button>
                      )
                    : null}
                  {thumbnailError
                    ? <FieldError>{thumbnailError}</FieldError>
                    : null}
                </div>
              </div>
            </Field>
          </FieldGroup>
        </form>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={isSubmitting}
          >
            Cancel
          </Button>
          <Button type="submit" form={formId} disabled={isSubmitting}>
            {isSubmitting
              ? "Saving..."
              : isEdit
                ? "Save changes"
                : "Create playlist"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
