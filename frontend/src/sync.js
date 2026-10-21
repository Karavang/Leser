import axios from "axios";
import { isOffline } from "./offline";

const KEY = "unsentPages";

const unsentAll = () => JSON.parse(localStorage.getItem(KEY)) || {};

const save = (all) => localStorage.setItem(KEY, JSON.stringify(all));

export const unsentPage = (id) => unsentAll()[id];

export const sendPage = (id, page) => {
  const token = JSON.parse(localStorage.getItem("token"));
  if (!token) return Promise.resolve();
  if (isOffline()) {
    save({ ...unsentAll(), [id]: page });
    return Promise.resolve();
  }
  return axios
    .post(
      "/api/pageWasFlipped",
      { book: id, page },
      { headers: { Authorization: `Bearer ${token}` } },
    )
    .then(() => {
      const all = unsentAll();
      if (all[id] === page) {
        delete all[id];
        save(all);
      }
    })
    .catch((error) => {
      if (error.response) return;
      save({ ...unsentAll(), [id]: page });
    });
};

export const sendUnsent = () =>
  !isOffline() &&
  Object.entries(unsentAll()).forEach(([id, page]) => sendPage(id, page));
