import { t } from "../i18n";

export const SearchingLine = ({ setFilter }) => {
  return (
    <input
      type="search"
      placeholder={t("searchPlaceholder")}
      className="searchingLine"
      onChange={(e) => setFilter(e.target.value)}
    />
  );
};
