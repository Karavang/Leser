import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ToastContainer, toast } from "react-toastify";
import "react-toastify/dist/ReactToastify.css";

const Auth = () => {
  const navigate = useNavigate();
  const emailPattern = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/;
  const passwordPattern = /^[A-Za-z0-9]{8,}$/;
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [inputType, setInputType] = useState("password");
  const [isRegister, setIsReg] = useState(true);
  const [isPassValid, setPassVal] = useState(false);
  const [isEmailValid, setEmailVal] = useState(false);
  const [isCanLogin, setLogin] = useState(false);

  // Вход и регистрация отличаются только адресом и телом, ответ один и тот же.
  const authenticate = async (path, body) => {
    try {
      const response = await fetch(path, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const data = await response.json();

      // fetch не бросает на 4xx: без этой проверки неверный пароль падал
      // дальше по коду на data.data.user и «срабатывал» через catch.
      if (!response.ok) {
        toast.error(data.message ?? "Не удалось войти");
        return;
      }

      localStorage.setItem("token", JSON.stringify(data.data.user.token));
      localStorage.setItem("user", JSON.stringify(data.data.user));
      // replace: «назад» с библиотеки не должно возвращать на форму входа
      navigate("/", { replace: true });
    } catch (error) {
      console.error(path, error);
      toast.error("Сервер не отвечает");
    }
  };

  const login = () => authenticate("/api/login", { email, password });
  const registration = () =>
    authenticate("/api/registration", { username, email, password });

  const checkEmail = (email) => {
    if (emailPattern.test(email)) {
      document.getElementById("emailInput").style.borderColor = "#256b1b";
      setEmailVal(true);
      setEmail(email.toLowerCase());
    } else {
      document.getElementById("emailInput").style.borderColor = "red";
      setEmailVal(false);
    }
  };
  const checkPass = (pass) => {
    if (passwordPattern.test(pass)) {
      document.getElementById("passInput").style.borderColor = "#256b1b";
      setPassVal(true);
      setPassword(pass);
    } else {
      document.getElementById("passInput").style.borderColor = "red";
      setPassVal(false);
    }
  };
  const typePass = () => {
    setInputType("text");
    setTimeout(() => setInputType("password"), 1000);
  };

  const canLogin = () => {
    if (isEmailValid && isPassValid) {
      setLogin(true);
    } else {
      setLogin(false);
    }
  };
  useEffect(() => {
    canLogin();
    // canLogin пересоздаётся каждый рендер, эффект нужен только на смену валидности
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPassValid, isEmailValid]);
  return (
    <div className="auth">
      {isRegister ? (
        <>
          <h1>Login</h1>
          <input
            type="text"
            id="emailInput"
            placeholder="Email"
            onChange={(e) => {
              setTimeout(
                () => checkEmail(e.target.value),
                isCanLogin ? 0 : 3000,
              );
            }}
          />
          <input
            id="passInput"
            placeholder="Password"
            onChange={(e) => {
              typePass();
              setTimeout(
                () => checkPass(e.target.value),
                isCanLogin ? 0 : 3000,
              );
            }}
            type={inputType}
          />
          <p>
            Don&apos;t registered yet? Time to{" "}
            <span onClick={() => setIsReg(false)}>registration</span>!
          </p>
          <button
            id="loginButton"
            disabled={!isCanLogin}
            onClick={login}
          >
            Login
          </button>
        </>
      ) : (
        <>
          <h1>Registration</h1>
          <input
            type="text"
            placeholder="Username"
            onChange={(e) => setUsername(e.target.value)}
          />{" "}
          <input
            type="text"
            id="emailInput"
            placeholder="Enter your email"
            onChange={(e) => {
              setTimeout(
                () => checkEmail(e.target.value),
                isCanLogin ? 0 : 3000,
              );
            }}
          />
          <input
            id="passInput"
            placeholder="Create password, min 8 symbols"
            onChange={(e) => {
              typePass();
              setTimeout(
                () => checkPass(e.target.value),
                isCanLogin ? 0 : 3000,
              );
            }}
            type={inputType}
          />
          <p>
            Already have account?{" "}
            <span onClick={() => setIsReg(true)}>Login</span> now!
          </p>
          <button
            id="loginButton"
            disabled={!isCanLogin}
            onClick={registration}
          >
            Register
          </button>
        </>
      )}
      <ToastContainer
        position="top-center"
        autoClose={2000}
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

export default Auth;
