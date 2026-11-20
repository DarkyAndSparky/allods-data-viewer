import "./style.css";
import { start } from "./app";

const host = document.querySelector<HTMLDivElement>("#app");
if (host) void start(host, import.meta.env.BASE_URL);
