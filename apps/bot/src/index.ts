import { Bot } from "grammy";
import type { Audio } from "grammy/types";
import { postAudio } from "./rest";
import { trycatch } from "@repo/trycatch";

if (!process.env.BOT_TOKEN) {
  throw new Error("process.env.BOT_TOKEN is not set");
}

const bot = new Bot(process.env.BOT_TOKEN);

bot.command("start", (ctx) => {
  ctx.reply("Hey! Send me MP3 files 🎵");
});

bot.on("message:audio", async (ctx) => {
  const audio: Audio = ctx.message.audio;

  const payload = {
    tg_user_id: ctx.from.id,
    ...audio,
  };

  const [response, error] = await trycatch(
    async () => await postAudio(payload),
  );

  if (error) {
    await ctx.reply("Error at adding audio: " + error.message);
    return;
  }

  if (!response.ok) {
    await ctx.reply("Error at response: " + response.statusText);
    return;
  }

  const reply = `
  Audio added successfully:
  Added: ${payload.title}
  Data: ${JSON.stringify(payload)}
  Audio obj: ${JSON.stringify(audio)}
  `;
  await ctx.reply(reply);
});

bot.start({
  onStart: (botInfo) => {
    console.log(`Bot started as ${botInfo.username}`);
  },
});
