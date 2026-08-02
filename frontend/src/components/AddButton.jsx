import { useState } from "react";
import { ModalAdd } from "./ModalAdd";

export const AddButton = ({ onAdded }) => {
  const [isAddModal, setIsAddModal] = useState();

  return (
    <>
      <button
        className="filtrationAndAdd"
        onClick={() => {
          setIsAddModal(true);
          document.body.style.overflowY = "hidden";
        }}
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          version="1.1"
          width="24"
          height="24"
          viewBox="0 0 512 512"
        >
          <g id="icomoon-ignore"></g>
          <path d="M446.134 193.245c1.222-5.555 1.866-11.324 1.866-17.245 0-44.183-35.817-80-80-80-7.111 0-14.007 0.934-20.566 2.676-12.399-38.676-48.645-66.676-91.434-66.676-43.674 0-80.527 29.168-92.163 69.085-11.371-3.311-23.396-5.085-35.837-5.085-70.692 0-128 57.308-128 128 0 70.694 57.308 128 128 128h64v96h128v-96h112c44.183 0 80-35.816 80-80 0-39.36-28.427-72.081-65.866-78.755zM288 320v96h-64v-96h-80l112-112 112 112h-80z" />
        </svg>
      </button>
      {isAddModal && (
        <ModalAdd
          setIsAddModal={setIsAddModal}
          onAdded={onAdded}
        />
      )}
    </>
  );
};
