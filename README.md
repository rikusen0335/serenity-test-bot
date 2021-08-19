# うんこ

コードめっちゃ汚いけどRust初めて書いたからお許しください

## ビルドとかに必要なライブラリ

開発機がFedoraだからほかは知らんけど、Fedoraではこいつらが必要だった。ワンチャン足りないかも

```
sudo dnf install libtool autoconf automake m4 opus ffmpeg
```

## 起動してみる

先に`docker-compose up`してOpenJTalkを立ち上げておく。

OSの環境変数にDiscordのトークンを設定しておく
```
export DISCORD_TOKEN="unkoburitoken"
```

あとは`cargo run`すれば勝手にビルドして、Botを起動してくれるはず
