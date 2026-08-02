import { FallingLines } from "react-loader-spinner";
import { t } from "../i18n";

export const Loading = () => {
  return (
    <div className="loading">
      <FallingLines
        height="512"
        width="512"
        radius="9"
        color="green"
        ariaLabel="three-dots-loading"
        wrapperStyle
        wrapperClass
      />
      <h1>{t("loading")}</h1>
    </div>
  );
};
