import { S3ConfigFormList } from "../components/ConfigFormList";
import Layout from "../components/Layout";

export default function Settings() {
  console.log("render")
  return (
    <Layout>
      <S3ConfigFormList />
    </Layout>
  );
}
