import { FallingLines } from "react-loader-spinner";

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
      <h1>Loading</h1>
    </div>
  );
};
