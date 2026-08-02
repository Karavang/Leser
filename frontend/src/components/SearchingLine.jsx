export const SearchingLine = ({ setFilter }) => {
  return (
    <input
      type="search"
      placeholder="Название, автор или серия"
      className="searchingLine"
      onChange={(e) => setFilter(e.target.value)}
    />
  );
};
