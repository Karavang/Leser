import axios from "axios";
import { useEffect, useState } from "react";
import { t } from "../i18n";

const ROLES = [
  ["admin", "roleAdmin"],
  ["full", "roleFull"],
  ["user", "roleUser"],
];

const inviteLink = (token) => `${location.origin}/login?invite=${token}`;

export const AdminPanel = ({ auth }) => {
  const [users, setUsers] = useState(null);
  const [invites, setInvites] = useState([]);
  const [copied, setCopied] = useState(null);
  const [error, setError] = useState(null);
  const fail = (e) => setError(e.response?.data?.message ?? t("loadFailed"));

  const load = () =>
    Promise.all([axios.get("/api/admin/users", auth), axios.get("/api/admin/invites", auth)])
      .then(([u, i]) => {
        setUsers(u.data);
        setInvites(i.data);
      })
      .catch((e) => e.response?.status !== 403 && fail(e));

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const copy = (token) =>
    navigator.clipboard?.writeText(inviteLink(token)).then(() => setCopied(token));

  const create = () =>
    axios
      .post("/api/admin/invites", null, auth)
      .then(({ data }) => {
        copy(data.token);
        return load();
      })
      .catch(fail);

  const drop = (token) =>
    axios.delete(`/api/admin/invites/${token}`, auth).then(load).catch(fail);

  const setRole = (id, role) =>
    axios
      .post(`/api/admin/users/${id}/role`, { role }, auth)
      .then(() => setUsers((list) => list.map((u) => (u.id === id ? { ...u, role } : u))))
      .catch(fail);

  // ponytail: the server decides who is an admin, so a session stored before a promotion still shows the panel.
  if (!users && !error) return null;

  return (
    <details className="notes admin">
      <summary>{t("admin")}</summary>
      {error && <p className="error">{error}</p>}

      <h4>{t("invites")}</h4>
      <p className="empty">{t("inviteHint")}</p>
      <button className="secondary" onClick={create}>
        {t("newInvite")}
      </button>
      <ul className="wordList">
        {invites.map((i) => (
          <li key={i.token}>
            {i.usedAt ? (
              <span>{t("usedBy", i.usedBy ?? "—")}</span>
            ) : (
              <>
                <b>
                  <input readOnly value={inviteLink(i.token)} onFocus={(e) => e.target.select()} />
                </b>
                <button className="secondary" onClick={() => copy(i.token)}>
                  {copied === i.token ? t("copied") : t("copyLink")}
                </button>
              </>
            )}
            <button title={t("delete")} onClick={() => drop(i.token)}>
              ×
            </button>
          </li>
        ))}
      </ul>

      <h4>{t("readers")}</h4>
      <ul className="wordList">
        {users?.map((u) => (
          <li key={u.id}>
            <b>{u.username}</b>
            <span>{u.email}</span>
            <select
              className="langPick"
              value={u.role}
              disabled={u.me}
              onChange={(e) => setRole(u.id, e.target.value)}
            >
              {ROLES.map(([role, label]) => (
                <option key={role} value={role}>
                  {t(label)}
                </option>
              ))}
            </select>
          </li>
        ))}
      </ul>
    </details>
  );
};
