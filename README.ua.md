# DayZ CMD

**dayz-cmd** — це експериментальний лаунчер (браузер серверів і засіб запуску)
[DayZ][] в [Steam][221100] [Proton][] для Linux.

<!-- rule: current lang, other langs sorted by alpha -->
> Цей документ доступний у мовах: [ua 🇺🇦][], [eng 🇬🇧][], [rus 🇷🇺][]

![logo][]

На момент реалізації цього проекту [Bohemia Interactive][] все ще не змогла
зробити робочий лаунчер для гри, який міг би коректно встановлювати
модифікації та підключаться до ігрових серверів. З цієї причини виник цей
проект.

Основні особливості:

* Оглядач серверів з інформацією про кожен сервер
* Нечіткий пошук в браузері серверів на базі [fzf][]
* Автоматична установка модів (як опція)
* Широкий набір фільтрів для пошуку серверів (карта, час доби, модифікації,
  кількість гравців, від першої особи, пароль тощо)
* Додаткова інформація у вигляді країни розташування (використовуючи geoip
  базу) та ping для кожного сервера
* Список обраного, історія останніх 10 ігор та створення ярликів швидкого
  запуску для підключення до серверів
* Оффлайн режим [DayZCommunityOfflineMode][] з автоматичною установкою,
  оновленням та можливістю вибору модифікацій
* Меню конфігурації з параметрами запуску гри, налаштуваннями лаунчера,
  керуванням та статистикою за модами
* Надає посилання з детальною інформацією про сервер на [battlemetrics][]

Окреме спасибі [dayz-linux-cli-launcher][] за ідею та [dayzsalauncher][] за
API.

## Попередній перегляд

> ![Демонстрація лаунчера](extra/dayz-cmd-demo.svg)
> **Демонстрація лаунчера**

<!-- markdownlint-disable -->
<details>
<summary>Більше скріншотів 👈</summary>
<div style="text-align:center">
<table border="0" cellspacing="0" cellpadding="0" style="border: none">
<tr>
  <td><img loading="lazy" width="100%" src="extra/s_main.png"/><p>Головне меню</p></td>
  <td><img loading="lazy" width="100%" src="extra/s_servers.png"/><p>Браузер серверів</p></td>
</tr>
<tr>
  <td><img loading="lazy" width="100%" src="extra/s_servers_filter.png"/><p>Фільтрування серверів</p></td>
  <td><img loading="lazy" width="100%" src="extra/s_servers_filter_map.png"/><p>Фільтрування по карті</p></td>
</tr>
<tr>
  <td><img loading="lazy" width="100%" src="extra/s_servers_filter_applied.png"/><p>Застосування фільтра</p></td>
  <td><img loading="lazy" width="100%" src="extra/s_servers_favorites.png"/><p>Браузер вибраного</p></td>
</tr>

<tr>
  <td><img loading="lazy" width="100%" src="extra/s_servers_history.png"/><p>Браузер уподобань</p></td>
  <td><img loading="lazy" width="100%" src="extra/s_servers_search.png"/><p>Нечеткий поиск</p></td>
</tr>
<tr>
  <td><img loading="lazy" width="100%" src="extra/s_offline.png"/><p>Оффлайн режим</p></td>
  <td><img loading="lazy" width="100%" src="extra/s_offline_mods.png"/><p>Моди для офлайн</p></td>
</tr>
<tr>
  <td><img loading="lazy" width="100%" src="extra/s_servers_mods.png"/><p>Моди сервера</p></td>
  <td><img loading="lazy" width="100%" src="extra/s_mods.png"/><p>Інформація про моди</p></td>
</tr>
<tr>
  <td><img loading="lazy" width="100%" src="extra/s_config.png"/><p>Меню конфігурації</p></td>
  <td><img loading="lazy" width="100%" src="extra/s_config_launch.png"/><p>Параметри запуску</p></td>
</tr>
<tr>
  <td><img loading="lazy" width="100%" src="extra/s_about.png"/><p>Інформація</p></td>
  <td><img loading="lazy" width="100%" src="extra/s_news.png"/><p>Новини DayZ</p></td>
</tr>
</table>
</div>
</details>
<!-- markdownlint-enable -->

## Особливості використання SteamCMD

Є два режими роботи лаунчера з використанням SteamCMD для керування модами
та без нього в ручному режимі.

Ви можете комбінувати обидва підходи, наприклад підписуватися на ті
модифікації, які вам точно потрібні будуть у майбутньому, переходячи за
посиланням, а наявність оновлень перевіряти або примусово оновлювати моди за
допомогою лаунчера. Також ви можете не підписуватися на "сумнівні 50 модів"
чергового сервера і легко видалити їх однією дією з лаунчера, при цьому
зберігши всі моди на які є підписка.

### Використовуючи SteamCMD

* 🟢 Все відбувається автоматично
* 🟢 Автоматична перевірка наявності оновлень модів прямо зараз (примусово)
* 🟡 Не створюється підписки на моди, вони просто скачуються
* 🟡 Вимагає закриття клієнта Steam для завантаження модів
* 🔴 Іноді потрібно повторно авторизуватися в Steam

### Не використовуючи SteamCMD

* 🟢 Звична поведінка якщо ви вже використовували інші рішення, наприклад
  [dayz-linux-cli-launcher][]
* 🟡 Гра сама не запуститься після завантаження модів
* 🟡 Steam буває затримує перевірку оновлень і скачує їх тільки після
  перезапуску або підписки/відписки від мода
* 🔴 Підписуватися на моди потрібно самому руками

## Встановлення

### За допомогою установника

Для зручності встановлення є невеликий скрипт, який зробить все за вас
(принаймні спробує зробити)

Виконайте це:

``` bash
curl -sSfLA dayz-cmd bit.ly/3Vf2zz3 | bash
```

## Ручна установка

Для роботи лаунчера вам необхідно переконатися, що у вас встановлені всі
залежності:

* [jq][] - утиліта для обробки JSON
* [fzf][] - утиліта для нечіткого пошуку
* [gum][] - утиліта для створення діалогів та стилізації виводу
* `ping` (`iputils-ping`) - дізнаємось пінг до сервера (де включений ICMP)
* `geoiplookup` (`geoip-bin`) - дізнаємось країну розміщення сервера
* `whois` - запасний варіант для geoiplookup, менш точний і повільніший, але
  не всі записи є в стандартній БД geoip
* `curl` - утиліта для комунікації з різними API з HTTP/S
* `cut, tr, grep, pgrep, pkill, killal, timeout, sed, awk` (`gawk`) - куди ж
  без класичних утиліт у скриптах
* [Steam][] - онлайн-сервіс цифрового розповсюдження комп'ютерних ігор
* [SteamCMD][] - steamcmd консольний клієнт Steam
* [DayZ][221100] - і природно сама гра

Після цього можете клонувати репозиторій:

``` bash
git clone git@github.com:WoozyMasta/dayz-cmd.git
# or
git clone https://github.com/WoozyMasta/dayz-cmd.git
# and run
cd dayz-cmd
./dayz-cmd
```

Або завантажити сам файл скрипту:

``` bash
curl -sSfL -o ~/.local/bin/dayz-cmd \
  https://raw.githubusercontent.com/WoozyMasta/dayz-cmd/master/dayz-cmd
chmod +x ~/.local/bin/dayz-cmd
# and run
dayz-cmd
```

### Emoji

Для нормального відображення елементів використовуються emoji, можливо
додатково вам знадобиться встановити їх, наприклад, ви можете
використовувати [Noto][] шрифт від Google.

Нижче наведено список із назвою пакета для різних дистрибутивів:

* `fonts-noto-color-emoji` - debian/ubuntu
* `google-noto-emoji-color-fonts` - centos
* `google-noto-emoji-fonts` - fedora
* `noto-fonts-emoji` - arch
* `font-noto-emoji` - alpine
* `noto-coloremoji-fonts` - suse

Або якщо вам не подобаються emoji або ви не можете використовувати їх з
якоїсь причини, ви можете застосувати патч для заміни їх на рядки:

```bash
sed -e 's/▫️/•/g' -e 's/🟩/✕/g' -e 's/⬛/ /g' -e 's/🕒/time/g' -e 's/❔/?/g' \
  -e 's/🟢/ok/g' -e 's/🔴/no/g' -e 's/🌙/night/g' -e 's/☀️/day/g' \
  -e 's/🔒/yes/g' -e 's/🔓/no/g' -e 's/✅/ok/g' -e 's/❌/no/g' \
  -i "$(which dayz-cmd)"
```

## Перевірялося у дистрибутивах

* 🟢 Debian bookworm
* 🟢 Debian bullseye
* 🟢 Debian buster
* 🟢 Ubuntu 18.04
* 🟢 Ubuntu 20.04 💯
* 🟢 Ubuntu 22.04 💯
* 🟢 Fedora latest
* 🟡 Centos 7 (small bugs)
* 🟡 Centos stream9 (small bugs)
* 🟢 Alpine latest
* 🟢 Alpine edge
* 🟢 Archlinux
* 🟡 Opensuse leap (small bugs)

## Інше

### Steam

Краще прибирати всі параметри запуску DayZ у Steam та керувати ними з
лаунчера чи навпаки. Так як ключі можуть дублюватися і це може викликати
плутанину, або в гіршому випадку обріже частину ключів, адже рядок
аргументів має ліміт довжини, а на серверах з великою кількістю модів
використовується і так дуже довгий параметр запуску.

Тобто. залиште параметри запуску порожніми, або вкажіть лише потрібний вам
набір допоміжних утиліт та змінних, наприклад:

``` bash
MANGOHUD=1 ENABLE_VKBASALT=1 gamemoderun %command%
```

### Синтаксис пошуку

Ви можете ввести кілька умов пошуку, розділених пробілами. наприклад
`^namalsk DE !PVE !RP`

<!-- markdownlint-disable MD013 -->

| Ключ      | Тип відповідності Опис                  |                                        |
| --------- | --------------------------------------- | -------------------------------------- |
| `sbtrkt`  | нечіткий збіг                           | Елементи, що відповідають `sbtrkt`     |
| `wild`    | точне співпадання (у лапках)            | Елементи, що включають `wild`          |
| `^music`  | точне співпадання префікса              | Елементи, що починаються з `music`     |
| `.mp3$`   | суфікс-точний збіг                      | Елементи, що закінчуються на `.mp3`    |
| `!fire`   | зворотне точне збіг                     | Предмети, які не містять слова `fire`  |
| `!^music` | точну відповідність зворотного префікса | Елементи, які не починаються з `music` |
| `!.mp3$`  | точне відповідність зворотного суфікса  | Елементи, що не закінчуються на `.mp3` |

<!-- markdownlint-enable MD013 -->

Термін з одним символом риси діє як оператор АБО

```regexp
PVE | RP
```

## Змінні оточення

Ви можете більш тонко керувати роботою лаунчера за допомогою змінних
оточення, які ви можете передавати в оточення як зазвичай так і записати у
файл конфігурації `$HOME/.local/share/dayz-cmd/dayz-cmd.conf` (за
замовчуванням)

Список доступних змінних:

* **`DAYZ_CMD_VERSION`** — версія програми
* **`DAYZ_CMD_NAME`**=`dayz-cmd` — назва програми
* **`DAYZ_GAME_ID`**=`221100` — ID гри в Steam
* **`APPLICATIONS_DIR`**=`$HOME/.local/share/applications` — каталог для
  зберігання ярликів додатків
* **`DAYZ_CMD_DIR`**=`$HOME/.local/share/dayz-cmd` — робочий каталог
  лаунчера
* **`DAYZ_CMD_BIN_DIR`**=`$HOME/.local/share/dayz-cmd/bin` — каталог
  зберігання додаткових виконуваних файлів
* **`DAYZ_REQUEST_TIMEOUT`**=`10` — стандартний тайм-аут для HTTP запитів у
  секундах
* **`DAYZ_CONFIG_FILE`**=`$DAYZ_CMD_DIR/$DAYZ_CMD_NAME.conf` —
  конфігураційний файл dayz-cmd
* **`DAYZ_SERVER_DB`**=`$DAYZ_CMD_DIR/servers.json` — файл бази серверів
* **`DAYZ_SERVER_DB_TTL`**=`300` — TTL для бази серверів у секундах
* **`DAYZ_SERVER_REQUEST_TIMEOUT`**=`30` — тайм одержання списку серверів у
  секундах
* **`DAYZ_NEWS_DB`**=`$DAYZ_CMD_DIR/news.json` — файл бази новин
* **`DAYZ_NEWS_DB_TTL`**=`3600` — TTL для бази новин в секундах
* **`DAYZ_MODS_DB`**=`$DAYZ_CMD_DIR/mods.json` — файл бази модифікацій
* **`DAYZ_PROFILE`**=`$DAYZ_CMD_DIR/profile.json` — файл профілю користувача
* **`DAYZ_HISTORY_SIZE`**=`10` — розмір історії для оглядача серверів
* **`DAYZ_FZF_HISTORY`**=`$DAYZ_CMD_DIR/.$DAYZ_CMD_NAME-history` — файл
  історії для нечіткого пошуку
* **`DAYZ_USERAGENT`**=`"$DAYZ_CMD_NAME $DAYZ_CMD_VERSION"` — `User-Agent`
  використовується при HTTP запитах
* **`DAYZ_API`**=`https://dayzsalauncher.com/api/v1` — адреса
  [API][dayzsalauncher] для отримання списку серверів
* **`DAYZ_STEAMCMD_ENABLED`**=`true` — перемикач для включення або
  відключення використання [SteamCMD][]
* **`DAYZ_FILTER_MOD_LIMIT`**=`10` — величина фільтру ліміту модів за
  умовчанням
* **`DAYZ_FILTER_PLAYERS_LIMIT`**=`50` — величина фільтру ліміту гравців за
  умовчанням
* **`DAYZ_FILTER_PLAYERS_SLOTS`**=`60` — величина фільтру ліміту слотів для
  гравців за умовчанням

## Корисне

* <https://github.com/FeralInteractive/gamemode> - може допомогти з
  продуктивністю гри
* <https://github.com/flightlessmango/MangoHud> — виведення інформації про
  використання ресурсів та дозволяє обмежувати частоту кадрів
* <https://github.com/DadSchoorse/vkBasalt> — покращення зображення, додає
  чіткості картинці
* <https://github.com/crosire/reshade-shaders> — додаткові шейдери, які
  можуть використовуватися з vkBasalt
* <https://github.com/StuckInLimbo/OBS-ReplayBuffer-Setup> — налаштування
  запису повторів в OBS
* <https://github.com/matanui159/ReplaySorcery> — утиліта для запису
  повторів

Параметри запуску гри в Steam c включеним MangoHud, vkBasalt та gamemode:

```sh
MANGOHUD=1 ENABLE_VKBASALT=1 gamemoderun %command%
```

Також не вдаючись до сторонніх утиліт ви можете вивести оверлей з
інформацією про ресурси і обмежити FPS штатними засобами [DXVK][],
наприклад:

```sh
DXVK_HUD=fps DXVK_FRAME_RATE=60 gamemoderun %command%
```

Значення `DXVK_HUD=fps` ... `DXVK_HUD=full`:

* `devinfo` — відображає назву GPU та версію драйвера.
* `fps` - показує поточну частоту кадрів.
* `frametimes` - показує часовий графік кадру.
* `submissions` — показує кількість командних буферів, надісланих на кадр.
* `drawcalls` - показує кількість викликів малювання та проходів рендерингу
  на кадр.
* `pipelines` - показує загальну кількість графічних і обчислювальних
  конвеєрів.
* `descriptors` - показує кількість пулів дескрипторів і наборів
  дескрипторів.
* `memory` — показує обсяг виділеної та використаної пам’яті пристрою.
* `gpuload` - показує приблизне навантаження GPU. Може бути неточним.
* `version` - Показує версію DXVK.
* `api` - показує рівень функцій D3D, який використовує програма.
* `cs` - Показує статистику робочого потоку.
* `compiler` - показує діяльність компілятора шейдера
* `samplers` — показує поточну кількість використаних пар семплерів [лише
  D3D9]
* `scale=x` - масштабує HUD за коефіцієнтом x (наприклад, 1,5)

Обмеження частоти кадрів  `DXVK_FRAME_RATE=0`

<!-- Links -->
[eng 🇬🇧]: README.md
[ua 🇺🇦]: README.ua.md
[rus 🇷🇺]: README.ru.md
[logo]: extra/dayz-cmd-logo.svg

[DayZ]: https://dayz.com
[Bohemia Interactive]: https://www.bohemia.net/games/dayz
[221100]: https://store.steampowered.com/app/221100
[dayz-linux-cli-launcher]: https://github.com/bastimeyer/dayz-linux-cli-launcher
[dayzsalauncher]: https://dayzsalauncher.com
[battlemetrics]: https://www.battlemetrics.com
[SteamCMD]: https://developer.valvesoftware.com/wiki/SteamCMD
[fzf]: https://github.com/junegunn/fzf
[jq]: https://github.com/stedolan/jq
[gum]: https://github.com/charmbracelet/gum
[DayZCommunityOfflineMode]: https://github.com/Arkensor/DayZCommunityOfflineMode
[Steam]: https://store.steampowered.com/about/
[Proton]: https://github.com/ValveSoftware/Proton
[Noto]: https://fonts.google.com/noto
[DXVK]: https://github.com/doitsujin/dxvk

<!--
DayZ DayZSA dayzstandalone dayz standalone linux nix proton steam
DayZ launcher Linux
DayZ servers browser linux
Дейз лаунчер Лінукс
DayZ Лінукс
DayZ Steam Proton
-->
