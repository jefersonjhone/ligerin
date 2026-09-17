import { useState } from "react";
import { lpc } from "./api";
import Login from "./components/Login";
import Header from "./components/Header";
import MotoristaHome from "./components/MotoristaHome";

function App() {
  const [token, setToken] = useState("");
  const [usuario, setUsuario] = useState("");
  const [idUsuario, setIdUsuario] = useState(0);
  const [error, setError] = useState("");

  function aoEntrar(token: string, usuario: string, id: number) {
    setToken(token);
    setUsuario(usuario);
    setIdUsuario(id);
  }

  async function sair() {
    try {
      setError("");
      await lpc(token, "POST", ".logout", {});
    } catch (err) {
      setError(String(err));
    }
    setToken("");
    setIdUsuario(0);
  }

  if (!token) {
    return <Login profiletype="motorista" onEntrar={aoEntrar} />;
  }

  return (
    <>
      <Header usuario={usuario} onSair={sair} />
      <MotoristaHome token={token} usuarioId={idUsuario} />
      {error && <p className="error">{error}</p>}
    </>
  );
}

export default App;