import { TelegramLoginButton } from "../../components/TelegramLoginButton";
import styles from "./page.module.css";

export default function LoginPage() {
  const botName = process.env.BOT_USERNAME ?? "your_bot";

  return (
    <div className={styles.container}>
      <div className={styles.card}>
        <div className={styles.logo}>SoundGrammy</div>
        <p className={styles.subtitle}>
          Your personal music library from Telegram
        </p>
        <TelegramLoginButton botName={botName} />
      </div>
    </div>
  );
}
