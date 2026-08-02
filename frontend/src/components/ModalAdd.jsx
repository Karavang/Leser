import axios from "axios";
import { useEffect } from "react";
import { FileUploader } from "react-drag-drop-files";
import { ToastContainer, toast } from "react-toastify";
import "react-toastify/dist/ReactToastify.css";

// Тот же список, что BOOK_EXTS в api/src/books.rs — здесь он только чтобы
// в диалоге выбора файла не показывались форматы, которые всё равно отлетят.
// rda — формат данных R, читалки под него нет: бэк превращает его в fb2.
const BOOK_TYPES = ["epub", "fb2", "rda"];
const MAX_MB = 64;

export const ModalAdd = ({ setIsAddModal, onAdded }) => {
  const user = JSON.parse(localStorage.getItem("user"));

  // Слушатель вешался прямо в теле компонента: новый на каждый рендер и ни
  // одного снятия. useEffect навешивает один раз и убирает за собой.
  useEffect(() => {
    const onKey = (e) => e.key === "Escape" && setIsAddModal(false);
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [setIsAddModal]);

  const handleFileChange = (event) => {
    const file = event;
    if (file) {
      const formData = new FormData();
      formData.append("file", file);

      const posting = new Promise((resolve, reject) => {
        axios
          .post("/api/putNewOne", formData, {
            headers: {
              "Content-Type": "multipart/form-data",
              Authorization: `Bearer ${user.token}`,
            },
          })
          .then((response) => {
            // книга уже в базе — список обновляем сразу, а не к перезагрузке
            onAdded?.();
            resolve(response.data);
          })
          .catch((error) => {
            // при сетевой ошибке response нет вовсе — без ?. тут падало
            reject(error.response?.data?.message ?? "Не удалось загрузить книгу");
          });
      });
      toast.promise(posting, {
        pending: "Загружаем книгу",
        success: "Книга добавлена",
        // сообщение приходит от бэка: битый файл, не тот формат, слишком большой
        error: { render: ({ data }) => data },
      });
    }
  };
  return (
    <div className="modalAdd">
      <div className="inModal">
        <div>
          <FileUploader
            className="loaderPlugin"
            handleChange={handleFileChange}
            name="file"
            types={BOOK_TYPES}
            maxSize={MAX_MB}
            label="Выберите книгу или перетащите её сюда"
            onTypeError={() =>
              toast.error(`Читаются только ${BOOK_TYPES.join(" и ")}`)
            }
            onSizeError={() => toast.error(`Книга больше ${MAX_MB} МБ`)}
          />
        </div>

        <button
          onClick={() => {
            setIsAddModal(false);
            document.body.style.overflowY = "scroll";
          }}
        >
          <svg
            version="1.1"
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 32 32"
          >
            <path d="M31.708 25.708c-0-0-0-0-0-0l-9.708-9.708 9.708-9.708c0-0 0-0 0-0 0.105-0.105 0.18-0.227 0.229-0.357 0.133-0.356 0.057-0.771-0.229-1.057l-4.586-4.586c-0.286-0.286-0.702-0.361-1.057-0.229-0.13 0.048-0.252 0.124-0.357 0.228 0 0-0 0-0 0l-9.708 9.708-9.708-9.708c-0-0-0-0-0-0-0.105-0.104-0.227-0.18-0.357-0.228-0.356-0.133-0.771-0.057-1.057 0.229l-4.586 4.586c-0.286 0.286-0.361 0.702-0.229 1.057 0.049 0.13 0.124 0.252 0.229 0.357 0 0 0 0 0 0l9.708 9.708-9.708 9.708c-0 0-0 0-0 0-0.104 0.105-0.18 0.227-0.229 0.357-0.133 0.355-0.057 0.771 0.229 1.057l4.586 4.586c0.286 0.286 0.702 0.361 1.057 0.229 0.13-0.049 0.252-0.124 0.357-0.229 0-0 0-0 0-0l9.708-9.708 9.708 9.708c0 0 0 0 0 0 0.105 0.105 0.227 0.18 0.357 0.229 0.356 0.133 0.771 0.057 1.057-0.229l4.586-4.586c0.286-0.286 0.362-0.702 0.229-1.057-0.049-0.13-0.124-0.252-0.229-0.357z"></path>
          </svg>
        </button>
      </div>
      <ToastContainer
        position="top-center"
        autoClose={5000}
        hideProgressBar={false}
        newestOnTop={false}
        closeOnClick
        rtl={false}
        pauseOnFocusLoss
        draggable
        pauseOnHover
        theme="dark"
        transition:Bounce
      />
    </div>
  );
};
