import { Router, Routes, Route, hashIntegration } from "@solidjs/router";
import { lazy } from "solid-js";

const Index = lazy(() => import("./pages/Index"));
const Screenshot = lazy(() => import("./pages/Screenshot"));

export default function App() {
  return (
    <Router source={hashIntegration()}>
      <Routes>
        <Route path="/" component={Index} />
        <Route path="/screenshot" component={Screenshot} />
      </Routes>
    </Router>
  );
}
