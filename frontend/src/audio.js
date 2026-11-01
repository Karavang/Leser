export const AUDIO_TYPES = ["mp3", "m4a", "m4b", "aac", "ogg", "oga", "opus", "flac", "wav", "webm"];

export const isAudio = (book) => book?.ext === "audio" || book?.audio === true;

export const bookPath = (book, at) =>
  `/${isAudio(book) ? "listen" : "book"}/${book.id}${at == null ? "" : `?at=${at}`}`;

const two = (n) => String(n).padStart(2, "0");

export const clock = (seconds) => {
  const s = Math.max(0, Math.floor(seconds || 0));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h ? `${h}:${two(m)}:${two(s % 60)}` : `${m}:${two(s % 60)}`;
};

export const hoursMinutes = (seconds) => {
  const m = Math.max(1, Math.round((seconds || 0) / 60));
  return [Math.floor(m / 60), m % 60];
};

export const starts = (tracks) =>
  tracks.reduce((acc, t) => [...acc, acc[acc.length - 1] + (t.seconds || 0)], [0]);

export const spotOf = (tracks, frac) => {
  const at = starts(tracks);
  const total = at[at.length - 1];
  const want = Math.min(Math.max(frac, 0), 1) * total;
  for (let i = 0; i < tracks.length; i++) {
    if (want < at[i + 1] || i === tracks.length - 1) return [i, Math.min(want - at[i], tracks[i].seconds || 0)];
  }
  return [0, 0];
};

export const marks = (tracks) =>
  tracks.flatMap((track, i) =>
    track.chapters?.length
      ? track.chapters.map((c, n) => ({ track: i, start: c.start, title: c.title || `${track.title} · ${n + 1}` }))
      : [{ track: i, start: 0, title: track.title }],
  );

export const markAt = (list, track, time) => {
  let found = 0;
  list.forEach((m, n) => {
    if (m.track < track || (m.track === track && m.start <= time + 0.25)) found = n;
  });
  return found;
};

const natural = (a, b) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" });

export const audioGroups = (files) => {
  const singles = files.filter((f) => /\.m4b$/i.test(f.name)).map((f) => [f]);
  const rest = files.filter((f) => !/\.m4b$/i.test(f.name)).sort(natural);
  return rest.length ? [...singles, rest] : singles;
};

export const groupName = (files) => {
  const folder = files[0].webkitRelativePath?.split("/").slice(-2, -1)[0];
  if (folder) return folder;
  const stems = files.map((f) => f.name.replace(/\.[^.]+$/, ""));
  if (stems.length === 1) return stems[0];
  let common = stems[0];
  for (const s of stems) while (!s.startsWith(common)) common = common.slice(0, -1);
  common = common.replace(/[\s\d_.()[\]-]+$/u, "").trim();
  return common.length >= 3 ? common : stems[0];
};
