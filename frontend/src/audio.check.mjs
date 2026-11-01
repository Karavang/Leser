import assert from "node:assert/strict";
import { audioGroups, bookPath, clock, groupName, markAt, marks, spotOf } from "./audio.js";

const tracks = [
  { title: "One", seconds: 100, chapters: [] },
  { title: "Two", seconds: 300, chapters: [{ title: "a", start: 0 }, { title: "b", start: 120 }] },
];

assert.deepEqual(spotOf(tracks, 0), [0, 0]);
assert.deepEqual(spotOf(tracks, 0.125), [0, 50]);
assert.deepEqual(spotOf(tracks, 0.25), [1, 0]);
assert.deepEqual(spotOf(tracks, 0.5), [1, 100]);
assert.deepEqual(spotOf(tracks, 1), [1, 300]);
assert.deepEqual(spotOf(tracks, 7), [1, 300]);

const list = marks(tracks);
assert.deepEqual(list.map((m) => m.title), ["One", "a", "b"]);
assert.equal(markAt(list, 0, 99), 0);
assert.equal(markAt(list, 1, 10), 1);
assert.equal(markAt(list, 1, 119.9), 2);
assert.equal(markAt(list, 1, 200), 2);

assert.equal(clock(0), "0:00");
assert.equal(clock(65), "1:05");
assert.equal(clock(3725.9), "1:02:05");

assert.equal(bookPath({ id: "x", ext: "epub" }), "/book/x");
assert.equal(bookPath({ id: "x", ext: "audio" }, 0.5), "/listen/x?at=0.5");
assert.equal(bookPath({ id: "x", audio: true }, "0.1"), "/listen/x?at=0.1");

const f = (name) => ({ name });
assert.deepEqual(
  audioGroups([f("10 end.mp3"), f("a.m4b"), f("2 two.mp3"), f("1 one.mp3")]).map((g) => g.map((x) => x.name)),
  [["a.m4b"], ["1 one.mp3", "2 two.mp3", "10 end.mp3"]],
);
assert.equal(groupName([f("Dracula 01.mp3"), f("Dracula 02.mp3")]), "Dracula");
assert.equal(groupName([f("01.mp3"), f("02.mp3")]), "01");
assert.equal(groupName([{ name: "1.mp3", webkitRelativePath: "Books/Idiot/1.mp3" }]), "Idiot");

console.log("audio ok");
