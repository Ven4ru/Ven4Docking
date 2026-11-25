# Ven4Docking — подготовка телефонов по брендам (исследование)

Дата: 25.11.2026. Только исследование: ни один телефон не трогали, команды не запускали.

**Легенда**
- Пункт с URL — подтверждён этим источником (официальная справка производителя, документация Android/AOSP, dontkillmyapp.com или сторонний гайд; уровень доверия указан, где важно).
- **«не подтверждено»** — найти надёжный источник не удалось, либо источники противоречат друг другу, либо есть только слабый блог.
- Русские названия пунктов даны там, где они есть в русскоязычном источнике. Иначе указано «рус. название не подтверждено» и дано английское.
- Названия в меню зависят от модели, региона и версии оболочки. В UI приложения нужен совет «если пункта нет — найдите его поиском по настройкам».

---

## 0. Общее для всех брендов

### 0.1. Включить «Для разработчиков» и отладку по USB (базовая схема)

- Найти «Номер сборки» (Build number) и нажать на него 7 раз, пока не появится «Вы стали разработчиком» (`You are now a developer!`). На Android 4.2+ раздел скрыт по умолчанию. https://developer.android.com/studio/debug/dev-options
- Где лежит «Отладка по USB» (USB debugging) в «чистом» Android:
  - Android 9+: Settings > System > Advanced > Developer options > USB debugging;
  - Android 8.0–8.1: Settings > System > Developer options;
  - Android 7.1 и ниже: Settings > Developer options.
  https://developer.android.com/studio/debug/dev-options
- Русские названия в «чистом» Android: «О телефоне» → «Номер сборки», затем «Для разработчиков» → «Отладка по USB». Можно найти поиском по настройкам. https://lifehacker.ru/otladka-po-usb-na-android/amp/
- При первом подключении на телефоне появится окно «Разрешить отладку по USB?». Его нужно подтвердить; удобно отметить «Всегда разрешать с этого компьютера». https://www.tenorshare.com/ru/usb-debugging-android.html (сторонний сайт)
- В «Для разработчиков» есть «Не выключать экран» (Stay awake): экран не гаснет, пока телефон на зарядке. https://developer.android.com/studio/debug/dev-options
- «Отключить тайм-аут авторизации adb» (Disable adb authorization timeout). С Android 10 система сама отзывает разрешение adb, если компьютер не подключался 7 дней. Этот переключатель (минимальный срок — 1 день) отключает отзыв. Для панели, к которой Ven4Docking вернётся через месяцы, это полезно. https://gerrit.omnirom.org/plugins/gitiles/android_packages_apps_Settings/+/0a189b3abe1e26274799ed0f9779ddd05f275183%5E2 , https://technastic.com/adb-device-unauthorized-android-devices/ . Русское название не подтверждено.

### 0.2. Беспроводная отладка (Android 11+)

- Поддерживается на телефонах с Android 11 (API 30) и выше. Телефон и ПК должны быть в одной Wi-Fi-сети. Сопряжение — по QR-коду или коду: `adb pair ip:port`, затем подключение. Сопряжение делается один раз; сеть должна поддерживать mDNS. https://developer.android.com/tools/adb
- Android 10 и ниже: только через первое USB-подключение: `adb tcpip 5555`, затем `adb connect ip:5555`. https://developer.android.com/tools/adb
- Путь: «Для разработчиков» → «Беспроводная отладка» (Wireless debugging). На Android 16+: Settings > System > Developer options > Wireless debugging. https://developer.android.com/studio/debug/dev-options . Русское название («Отладка по Wi-Fi» / «Беспроводная отладка») зависит от оболочки — не подтверждено.
- Android 17 + adb 37.0.0 — «adb Wi-Fi 2.0»: в доверенной сети устройство подключается к ПК само. https://developer.android.com/tools/adb

### 0.3. Что меняется между Android 7 и 15

| Что | С какой версии | Источник |
|---|---|---|
| Утилита `cmd` (`cmd package`, `cmd appops`, `cmd deviceidle` и т. д.) | Android 7.0 (в 6.0 её нет) | https://android.googlesource.com/platform/frameworks/native/+/refs/tags/android-7.0.0_r1/cmds/cmd/ |
| `cmd package set-home-activity [--user N] TARGET-COMPONENT` | Есть уже в Android 7.0 | https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-7.0.0_r1/services/core/java/com/android/server/pm/PackageManagerShellCommand.java |
| `set-home-activity` принимает просто имя пакета (не только `pkg/.Activity`) | В поздних версиях AOSP. С какой именно — не подтверждено; для 7–9 надёжнее передавать полный компонент | https://gitlab.e.foundation/e/os/android_frameworks_base/-/commit/1b21124d126792e77cdac40c7342adb3486f004b |
| Роль HOME: `cmd role add-role-holder [--user N] android.app.role.HOME <pkg>` | Android 10 (появились роли) | https://source.android.com/docs/core/permissions/android-roles , https://gitlab.e.foundation/e/os/android_frameworks_base/-/commit/6260c58793fecbcee31168ea6ee2065059e24d83 |
| `locksettings` (`set-disabled`, `clear --old …`) | Android 8.0 (в 7.1.2 каталога `cmds/locksettings` нет) | https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-8.0.0_r1/cmds/locksettings/ |
| `locksettings set-disabled true` работает только без PIN/пароля; при защищённой блокировке нужен `locksettings clear --old <PIN>` | 8.0+ | https://www.esper.io/blog/how-to-disable-the-lock-screen-on-android |
| `cmd appops set` / `get` | Android 7.0 | https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-7.0.0_r1/services/core/java/com/android/server/AppOpsService.java |
| Операция `RUN_IN_BACKGROUND` (op 63) | Android 7.0 | https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-7.0.0_r1/core/java/android/app/AppOpsManager.java |
| Операция `RUN_ANY_IN_BACKGROUND` (op 70) — именно её выставляет Ven4Docking | **Только Android 9+.** На 7–8 команда упадёт; используйте `RUN_IN_BACKGROUND` | https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-9.0.0_r1/core/java/android/app/AppOpsManager.java , https://developer.android.com/topic/performance/power/test-power |
| `am set-standby-bucket <pkg> active` (группы App Standby) | Android 9 | https://developer.android.com/topic/performance/power/test-power |
| Doze и список исключений (`dumpsys deviceidle whitelist +pkg`; с 7.0 также `cmd deviceidle whitelist +pkg`) | Android 6.0 | https://developer.android.com/training/monitoring-device-state/doze-standby |
| `stay_on_while_plugged_in` — битовая маска: AC=1, USB=2, беспроводная=4, док=8 (док — API 33). «Всегда» = 7 (или 15 на 13+) | API 17+ | https://developer.android.com/reference/android/provider/Settings.Global#STAY_ON_WHILE_PLUGGED_IN , https://developer.android.com/reference/android/os/BatteryManager#BATTERY_PLUGGED_DOCK |
| Беспроводная отладка с сопряжением | Android 11 | https://developer.android.com/tools/adb |
| APK с `targetSdkVersion` < 23 не установится (`INSTALL_FAILED_DEPRECATED_SDK_VERSION`); обход — `adb install --bypass-low-target-sdk-block` | Android 14 | https://developer.android.com/about/versions/14/behavior-changes-all |
| То же, но порог `targetSdkVersion` 24 | Android 15 | https://developer.android.com/about/versions/15/behavior-changes-all |
| «Ограниченные настройки» для приложений не из магазина: спецвозможности, доступ к уведомлениям, наложение. Обход — «⋮ → Разрешить ограниченные настройки» в сведениях о приложении. Действует ли это на `adb install` — **не подтверждено** | Android 13 | https://bayton.org/android/android-13-restricted-permissions/ , https://support.eset.com/en/kb8366-accessibility-restriction-on-android-13-for-apps-installed-from-apk-file |
| Принудительная остановка (Force stop) отменяет все PendingIntent; приложение «оживает» только после действия пользователя | Android 15 | https://developer.android.com/about/versions/15/behavior-changes-all |
| Лаунчер должен держать роль `ROLE_HOME` и объявить `ACCESS_HIDDEN_PROFILES`, чтобы видеть «Личное пространство» (Private space). Для киоска не критично | Android 15 | https://developer.android.com/about/versions/15/behavior-changes-all |

**WebView (важно для панели на веб-интерфейсе)**
- На Android 10+ Chrome больше не может быть реализацией WebView: работает отдельное «Android System WebView», которое обновляется через Play. На Android 7–9 реализацией WebView был Chrome. https://chromium.googlesource.com/chromium/src/+/HEAD/android_webview/docs/prerelease.md
- Провайдера WebView меняют в «Для разработчиков → Сервис WebView» (Developer options > WebView implementation) или командой `adb shell cmd webviewupdate set-webview-implementation <pkg>`. https://chromium.googlesource.com/chromium/src/+/HEAD/android_webview/docs/prerelease.md . Русское название пункта не подтверждено.
- Конец обновлений: для Android 7 последний Chrome — 119 (декабрь 2023). https://www.androidheadlines.com/2023/11/android-7-chrome-support.html , https://groups.google.com/a/chromium.org/g/chromium-dev/c/B9AYI3WAvRo . Для Android 8 и 9 последний — Chrome 138; с Chrome 139 (август 2025) минимум — Android 10. https://9to5google.com/2025/06/26/google-chrome-android-versions-no-longer-supported-2025/ , https://groups.google.com/a/chromium.org/g/chromium-dev/c/vEZz0721rUY
- WebView поддерживает те же версии Android, что и Chrome той же версии. Сейчас Chrome на Android требует Android 10+. https://chromium.googlesource.com/chromium/src.git/+/HEAD/docs/supported_platforms.md
- **Вывод для Ven4Docking:** на Android 7–9 движок WebView заморожен на старой версии и не получает исправлений безопасности. Веб-интерфейс панели должен работать без новых возможностей браузера, либо приложение должно предупреждать владельца.
- На Huawei без Google-сервисов WebView через Play не обновляется. Чем его обновлять (AppGallery?) — **не подтверждено**.

### 0.4. Фоновая работа: что даёт adb, а что нет

- Пока телефон заряжается, система выводит приложения из режима ожидания (App Standby), а Doze включается только без зарядки. Для панели на постоянной зарядке штатный Doze почти не мешает; главная угроза — собственные «убийцы» приложений в оболочках. https://developer.android.com/training/monitoring-device-state/doze-standby
- Исключение из оптимизации батареи частично снимает только Doze и App Standby (сеть и частичные wake lock'и). Остальные ограничения остаются. https://developer.android.com/training/monitoring-device-state/doze-standby
- **Через adb можно (все бренды, при условии из раздела 2 для Xiaomi и ColorOS):**
  - `cmd deviceidle whitelist +<pkg>` — исключение из оптимизации батареи;
  - `cmd appops set <pkg> RUN_ANY_IN_BACKGROUND allow` (9+), на 7–8 — `RUN_IN_BACKGROUND`;
  - `am set-standby-bucket <pkg> active` (9+).
  https://developer.android.com/topic/performance/power/test-power
- **Через adb нельзя (или не подтверждено):** OEM-переключатели — «Автозапуск» Xiaomi, «Запуск приложений» Huawei, «Неспящие приложения» Samsung, «Закрепить в недавних». Они хранятся в собственных базах оболочек, а не в `settings`/`appops`. Стандартной adb-команды для них не найдено — **не подтверждено**, что это возможно. Исключение: на Huawei и Motorola dontkillmyapp предлагает удалить системный «убийца» через `pm uninstall -k --user 0` (см. бренды). Это вмешательство в систему; по правилам продукта его нужно предлагать только с явным согласием.
- Закрепление в «Недавних» (значок замка) есть почти у всех китайских оболочек. Для лаунчера, который всегда на переднем плане, оно менее важно, но помогает фоновым службам. https://dontkillmyapp.com/

### 0.5. Лаунчер по умолчанию: общее

- Путь в «чистом» Android: Settings > Apps > Default apps > Home app. У Pixel: «Настройки → Приложения → Приложения по умолчанию». https://support.google.com/pixelphone/answer/6271667?hl=en , https://support.google.com/googleplay/thread/433278680/how-do-i-change-my-launcher-i-can-t-get-the-home-app-to-change-from-default?hl=en
- Через adb: `cmd package set-home-activity <pkg>/<activity>` (7+). На 10+ дополнительно `cmd role add-role-holder android.app.role.HOME <pkg>`. Проверить: `cmd package resolve-activity -a android.intent.action.MAIN -c android.intent.category.HOME` или `dumpsys role`. https://mustafa.net/2026/11/01/replace-google-tv-launcher-without-root/ , https://forum.fairphone.com/t/keeps-asking-which-launcher-to-use/87654?page=3
- Отсутствие ошибки не значит, что команда сработала: на Google TV стоковый лаунчер после этой команды всё ещё перехватывал HOME. Проверять надо всегда. https://mustafa.net/2026/11/01/replace-google-tv-launcher-without-root/
- Во многих сборках Android 10 жесты навигации отключаются, если по умолчанию стоит сторонний лаунчер. https://android.gadgethacks.com/how-to/force-android-10s-gestures-work-with-any-custom-launcher-0238065 . **Для панели советуем навигацию тремя кнопками** (или скрывать навигацию средствами самого киоск-приложения).

### 0.6. Работа 24/7 на стене: общие риски

- **Вздутие аккумулятора.** Есть много сообщений о вздутых батареях у планшетов, круглосуточно стоящих на зарядке (около года работы), в том числе как угрозе возгорания. Обычные меры: держать заряд 20–80 % реле или умной розеткой (Home Assistant/Shelly), использовать встроенный лимит заряда, регулярно осматривать устройство, выбирать устройства без батареи. https://community.openhab.org/t/bloated-battery-in-tablet-fire-hazard/145821 , https://community.home-assistant.io/t/need-advice-android-tablet-for-smart-home-display-wall-mounted-always-powered/843270 , https://acroname.com/blog/learn-how-mitigate-battery-swelling-mobile-device-labs . Это форумы и сообщения пользователей, а не исследования.
- **Выгорание экрана (OLED/AMOLED).** Статичные элементы при высокой яркости выжигаются, и это необратимо. Меры: яркость ≤ 50 %, тёмная тема, без статичных кнопок навигации, гашение экрана ночью или по датчику, сдвиг элементов. https://asurion.com/connect/tech-tips/oled-burn-in-how-to-avoid-and-fix-tv-and-phone-screen-burn , https://drfone.wondershare.com/android-samsung/how-to-fix-samsung-galaxy-screen-burn-in.html (сторонние). LCD-экраны (дешёвые Redmi, realme C, Tecno) почти не выгорают — **не подтверждено** конкретными источниками.
- **Встроенный лимит заряда.** Включается без root, но только из интерфейса телефона и только на моделях, где он есть (см. бренды). adb-команды для включения у Samsung, Xiaomi, Pixel и других **не подтверждены**. Безопасный путь — показать владельцу инструкцию.

---

## 1. Xiaomi / Redmi / POCO (MIUI 10–14, HyperOS 1–3)

### 1.1. Режим разработчика и отладка по USB
- «Настройки» → «О телефоне» → 7–10 раз нажать «Версия MIUI» (на HyperOS — плитку версии HyperOS / «Версия ОС»). https://yandex.ru/support/addappter/ru/work/device-control , https://miuirom.org/updates/usb-debugging
- Затем «Настройки» → «Расширенные настройки» (на старых MIUI — «Дополнительно») → «Для разработчиков» → «Отладка по USB». https://yandex.ru/support/addappter/ru/work/device-control , https://miuirom.org/updates/usb-debugging . В англ. интерфейсе: Settings > Additional settings > Developer options. Название «Расширенные настройки» на HyperOS — не подтверждено официальным источником.
- Если телефон «оптимизировать» приложением «Безопасность», вход в «Для разработчиков» может пропасть. https://www.mi.com/global/support/faq/details/KA-168765 (официальный FAQ; страница не открылась напрямую, текст по выдаче поиска)

### 1.2. Дополнительные переключатели для adb (обязательно)
- **«Установка через USB»** (Install via USB) — без него `adb install` падает с `INSTALL_FAILED_USER_RESTRICTED`. При включении просит войти в Mi-аккаунт. https://yandex.ru/support/addappter/ru/work/device-control , https://kiwix.lema.org/stackoverflow.com_en_all_2023-11/questions/57986113/install-failed-user-restricted-installation-via-usb-is-disabled
- **«Отладка по USB (настройки безопасности)»** (USB debugging (Security settings)) — без него `pm grant … WRITE_SECURE_SETTINGS`, а также запись настроек и разрешений из adb падают с «Permission denial». Перед включением телефон трижды показывает предупреждение с отсчётом 10 секунд. https://discuss.appium.io/t/permission-denial-writing-to-settings-requires-android-permission-write-secure-settings-on-xiaomi-device/32095 , https://fmd-foss.org/docs/fmd-android/granting-secure-settings-access
- **Mi-аккаунт:** нужен, чтобы включить эти пункты. https://yandex.ru/support/addappter/ru/work/device-control
- **SIM-карта:** по сторонним гайдам, «Установка через USB» включается только со вставленной SIM (после включения SIM можно вынуть), иногда нужен мобильный интернет вместо Wi-Fi. https://mobicode.freshdesk.com/support/solutions/articles/6000271953-xiaomi-usb-debugging-mode . Официально Xiaomi — **не подтверждено**; требование может зависеть от региона и прошивки.
- Ошибка «The device is temporarily restricted» — Яндекс советует сменить регион. https://yandex.ru/support/addappter/ru/work/device-connect-android.md
- Отключить «Проверять приложение при установке», если пункт есть. https://yandex.ru/support/addappter/ru/work/device-control
- Даже со всеми тремя переключателями на Xiaomi 15 с HyperOS 2.0.16 есть сообщение об ошибке выдачи разрешения. Это **не подтверждено** как массовая проблема. https://github.com/orgs/gkd-kit/discussions/767
- «Включить оптимизацию MIUI» (на HyperOS — «Turn on system optimization»): скрытый пункт внизу «Для разработчиков», появляется после нескольких нажатий «Восстановить значения по умолчанию». Может мешать фоновым задачам. https://dontkillmyapp.com/xiaomi , https://droidwin.com/?p=58295 . Рус. название на HyperOS не подтверждено.

### 1.3. Беспроводная отладка
- На прошивках с Android 11+ пункт находится в «Для разработчиков» (общий механизм Android). https://developer.android.com/tools/adb . Наличие и рус. название у Xiaomi отдельным источником — **не подтверждено**. Mi-аккаунт и переключатели из 1.2 нужны и здесь (логически следует из 1.2; отдельно не проверено).

### 1.4. Чтобы приложение не убивали
- MIUI 13+/HyperOS: «Настройки» → «Приложения» → «Разрешения» → «Автозапуск в фоновом режиме» → включить. В карточке приложения: «Экономия заряда батареи» (на HyperOS — «Контроль активности») → «Без ограничений»/«Нет ограничений»; отключить «Приостановить работу в неактивный период». https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/ , https://dontkillmyapp.com/xiaomi
- MIUI 12: «Управление приложениями» → приложение → «Автозапуск»; «Контроль фоновой активности» → «Нет ограничений». MIUI 10–11: «Батарея и производительность» → выключить «Экономия энергии». https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/
- Закрепить в недавних: потянуть карточку вниз или удерживать её и нажать замок. Иначе — «Безопасность» → «Ускорение» → шестерёнка → «Закрепить приложения». https://dontkillmyapp.com/xiaomi , https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/
- Через adb: из «Автозапуска» и «Контроля активности» ничего не выставляется (не подтверждено, что вообще возможно). Работают только стандартные команды из 0.4, и только после включения «Отладки по USB (настройки безопасности)».
- После крупных обновлений оболочка может сбросить разрешения — проверять заново. https://www.securitylab.ru/blog/personal/xiaomite-journal/362848.php

### 1.5. Лаунчер по умолчанию
- Со сторонним лаунчером HyperOS и MIUI отключают полноэкранные жесты («Can't use full screen gestures with third party launchers»). Жесты встроены в стоковый лаунчер; есть только неофициальные обходы. https://beebom.com/trick-use-gestures-third-party-launchers-xiaomi-phones-hassle , https://piunikaweb.com/2021/01/28/android-11-update-full-screen-gestures-on-mi-10-poco-f2-pro-workaround/ . **Перед сменой лаунчера переключить навигацию на кнопки.**
- Сообщения сообщества: Xiaomi ограничивает смену лаунчера в настройках. https://support.google.com/googleplay/thread/433278680/how-do-i-change-my-launcher-i-can-t-get-the-home-app-to-change-from-default?hl=en (форум). Работа `cmd package set-home-activity` на HyperOS отдельно — **не подтверждено**; проверять через `resolve-activity`.

### 1.6. 24/7
- «Защита аккумулятора» (Battery protection): «Настройки» → «Батарея» → «Защита аккумулятора». Режим «Умная зарядка» (Smart charging) останавливает заряд на 80 %; «Полная зарядка» отключает ограничение (Xiaomi 15/15T, HyperOS). https://www.mi.com/uk/support/faq/details/KA-612092/ , https://www.mi.com/my/support/faq/details/KA-538544/ , https://www.mi.com/ph/support/faq/details/KA-539817/
- На старых и бюджетных моделях пункта может не быть (на Xiaomi 14 с HyperOS 2 его не было). https://drfone.wondershare.com/android-xiaomi/how-to-turn-on-off-xiaomis-80-charging-limit-optimized-charging.html (сторонний). Рус. названия пунктов — не подтверждены.

---

## 2. Samsung (One UI 1–8)

### 2.1. Режим разработчика и отладка по USB
- «Настройки» → «Сведения о телефоне» → «Сведения о ПО» → «Номер сборки» ×7. Англ.: Settings > About phone > Software information > Build number. https://developer.android.com/studio/debug/dev-options , https://skillbox.ru/media/code/android-devmode/
- Затем «Настройки» → «Параметры разработчика» → «Отладка USB». https://yandex.ru/support/addappter/ru/work/device-control
- На Android 10–13 отключить «Проверять приложения при установке», если пункт есть. https://yandex.ru/support/addappter/ru/work/device-control

### 2.2. Автоблокировка (Auto Blocker, One UI 6+) — главный барьер
- Путь: «Настройки» → «Безопасность и конфиденциальность» → «Автоблокировка». Внутри: «Блокировка приложений из неофициальных…», «Блокировка команд через USB-кабель», «Блокирует неофициальные обновления ПО»; отдельно — «Максимальные ограничения» (One UI 6.1.1+). https://www.samsung.com/uz_ru/support/mobile-devices/protect-your-galaxy-device-with-the-new-auto-blocker-feature
- Блокирует команды по USB от «зарядок, компьютеров и других устройств» и установку приложений не из Galaxy Store и Google Play. https://www.samsung.com/us/support/answer/ANS10003636
- По умолчанию: включена на устройствах, вышедших с One UI 6.1.1. После обновления до 6.1.1 сохраняется прежнее значение. https://www.samsung.com/us/support/answer/ANS10003636 , https://www.samsung.com/uz_ru/support/mobile-devices/protect-your-galaxy-device-with-the-new-auto-blocker-feature
- При включённой «Автоблокировке» пункт «Отладка USB» выключается и становится серым. Лечится только выключением «Автоблокировки»; иногда после этого нужна перезагрузка или повторное включение отладки. Так пишут пользователи на форуме Samsung и сторонние гайды; Samsung сам про отладку не пишет. https://forum.developer.samsung.com/t/s24-ultra-developer-option-usb-debugging-greyed-out/29526 , https://techwiser.com/fix-usb-debugging-greyed-out-on-samsung-galaxy-phone/
- **Порядок в Ven4Docking:** 1) выключить «Автоблокировку»; 2) включить «Отладку USB»; 3) после настройки можно предложить включить «Автоблокировку» обратно. Ломает ли это уже установленное соединение adb — **не подтверждено**.

### 2.3. Беспроводная отладка
- One UI 3+ (Android 11+): «Параметры разработчика» → «Беспроводная отладка» (общий механизм Android). https://developer.android.com/tools/adb . По сообщениям с форума, «Автоблокировка» блокирует и её. https://r2.community.samsung.com/t5/Discussions/Usb-wireless-debugging-options-disabled/td-p/14733680 — **не подтверждено** официально.

### 2.4. Чтобы приложение не убивали
- Android 11+: «Настройки» → «Приложения» → приложение → «Батарея» → «Не ограничено» (Unrestricted). https://www.samsung.com/kz_ru/support/mobile-devices/apps-stop-working-in-background-mode
- Android 12–14: «Настройки» → «Обслуживание устройства» → «Батарея» → «Ограничения в фоновом режиме» → «Приложения в режиме сна» → ⋮ → «Убрать». Там же выключить «Переводить неиспользуемые приложения в спящий режим» и добавить приложение в «Неспящие» (Never sleeping apps). https://www.samsung.com/kz_ru/support/mobile-devices/apps-stop-working-in-background-mode , https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/ , https://dontkillmyapp.com/samsung
- Android 10: «Мониторинг энергопотребления» → «Приложения в спящем режиме». https://www.samsung.com/kz_ru/support/mobile-devices/apps-stop-working-in-background-mode
- Android 7–8: «Обслуживание устройства» → «Батарея» → добавить в «Неконтролируемые приложения» (Unmonitored apps). https://dontkillmyapp.com/samsung
- Выключить «Адаптивный режим аккумулятора» и правила в «Батарея» → ⋮ → «Автоматизация». Закрепить в «Недавних» (удерживать значок → «Не закрывать»/Keep open). https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/ , https://dontkillmyapp.com/samsung
- Samsung обещала соблюдать политику foreground-служб с One UI 6.0; подтверждений нет. https://dontkillmyapp.com/samsung
- Через adb списки «спящих» приложений не меняются (не подтверждено, что возможно).

### 2.5. Лаунчер по умолчанию
- Путь: «Настройки» → «Приложения» → «Выбор приложений по умолчанию» → «Главный экран» — **рус. название не подтверждено**; англ. Settings > Apps > Choose default apps > Home app — тоже не подтверждено. На форумах пишут, что Samsung (как и Xiaomi) иногда ограничивает смену лаунчера. https://support.google.com/googleplay/thread/433278680
- Запрет жестов со сторонним лаунчером на Samsung — **не подтверждено**.

### 2.6. 24/7
- One UI 6.1+: «Настройки» → «Батарея» → «Защита аккумулятора». Режимы:
  - «Базовая» — пауза на 100 %, дозаряд с 95 %;
  - «Адаптивная» — ночью держит 80 %;
  - «Максимальная» — 80 %.
  https://www.samsung.com/kz_ru/support/mobile-devices/galaxy-battery-protection-feature-in-one-ui-6-1
- One UI 7+: лимит в «Максимальной» можно выбрать — 80/85/90/95 %. На One UI 8 он держит заряд между 80 и 95 %. https://www.samsung.com/ru/support/mobile-devices/what-are-specific-differences-of-battery-protection-options , https://www.samsung.com/ae/support/mobile-devices/what-are-the-specific-differences-of-the-battery-protection-options/
- До One UI 6.1 — только ограничение 85 % («Защита аккумулятора»/Protect battery). https://www.samsung.com/kz_ru/support/mobile-devices/galaxy-battery-protection-feature-in-one-ui-6-1 . Точный путь на старых версиях — «Батарея и обслуживание устройства» → «Батарея» → «Другие настройки батареи» — **не подтверждено**.
- **Для панели — «Максимальная» (80 %). Это главная рекомендация для Samsung.** Включение через adb (`settings … protect_battery`) — **не подтверждено**, не использовать вслепую.
- Почти все Galaxy с AMOLED: риск выгорания (см. 0.6).

---

## 3. OPPO / realme / OnePlus (ColorOS, realme UI, OxygenOS 12+)

### 3.1. Режим разработчика и отладка по USB
- OPPO, realme: «Настройки» → «О телефоне» (на новых — «Об устройстве» → «Версия») → «Номер сборки» ×7. Затем «Дополнительные настройки» → «Возможности разработчика» (realme/ColorOS; иногда «Для разработчиков») → «Отладка по USB». https://yandex.ru/support/addappter/ru/work/device-control
- На новых ColorOS после нажатий просит PIN блокировки, а «Для разработчиков» может оказаться в «Системные настройки». https://www.techbone.net/oppo/smartphone/enable-developer-options , https://android.gadgethacks.com/how-to/unlock-androids-developer-options-access-powerful-hidden-tools-anyone-can-use-0385359 (сторонние)
- OnePlus (OxygenOS 14): «Об устройстве» → «Версия» → «Номер сборки» ×7. https://www.techbone.de/oneplus/smartphone/entwickleroptionen-aktivieren?from=11r (сторонний). Официальной страницы OnePlus не найдено.

### 3.2. Дополнительные переключатели для adb
- **«Отключить контроль разрешений»** (Disable permission monitoring) — в «Возможностях разработчика», обычно в самом низу. Без него `pm grant WRITE_SECURE_SETTINGS`, `settings put/delete` и выдача разрешений из adb падают с `SecurityException`. Владелец проверил это на realme C11 (Android 10): нужно для `settings put` и `appops`. Источники:
  - официальная инструкция Яндекса для OPPO и realme: https://yandex.ru/support/addappter/ru/work/device-control
  - realme X2: https://groups.google.com/g/tasker/c/f9zVYjaZTKA
  - realme RMX3630 с Android 14: https://forum.katalon.com/t/how-to-resolve-permission-denial-write-secure-settings-error-when-starting-a-mobile-test-on-android-with-katalon/159132
  - OnePlus/ColorOS 11: https://sspai.com/post/67110
  - OPPO/OnePlus: https://karasevm.github.io/PrivateDNSAndroid
- После переключения может понадобиться перезагрузка. https://forum.powerampapp.com/topic/24744-granting-permission-via-adb-oneplus/
- Побочный эффект: перестаёт работать «Управление разрешениями» в «Диспетчере телефона». https://sspai.com/post/67110
- «Установка через USB» и проверка приложений по USB: на ColorOS бывает окно выбора режима USB и ограничения установки из adb. Ошибка `-99` на OPPO R11 (Android 7) с экраном входа в аккаунт описана, но решение не найдено. Необходимость пароля OPPO-аккаунта для `adb install` — **не подтверждено**. https://kiwix.lema.org/stackoverflow.com_en_all_2023-11/questions/45790414/oppo-failed-to-install-apk-failure-99 , https://github.com/topics/coloros?l=java

### 3.3. Беспроводная отладка
- ColorOS 11+ / realme UI 2+ (Android 11+): «Возможности разработчика» → «Беспроводная отладка» (общий механизм Android). https://developer.android.com/tools/adb . Наличие и название у этих брендов отдельно — **не подтверждено**. «Отключить контроль разрешений» нужен и здесь (по логике 3.2).

### 3.4. Чтобы приложение не убивали
- realme UI: приложение → «Использование батареи» (Battery usage) → включить «Разрешить автозапуск», «Разрешить активность в фоне», «Разрешить активность на переднем плане» (Allow auto launch / background / foreground activity). Выключить «Приостанавливать работу, если не используется» (Pause app activity if unused). https://dontkillmyapp.com/realme , https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/ (рус. названия не подтверждены — источник с англ. интерфейсом)
- Путь на новых версиях: «Настройки» → «Батарея» → «Настройки энергосбережения» → «Управление батареей приложений». Не добавлять приложение в «Быструю заморозку приложений» (App quick freeze); выключить «Оптимизацию режима сна» (Sleep standby optimization). https://dontkillmyapp.com/realme
- OPPO: «Центр безопасности» → «Разрешения на конфиденциальность» → «Менеджер запуска»; приложение → «Использование батареи» → «Разрешить фоновую активность»; «Автозапуск». https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/ , https://dontkillmyapp.com/oppo
- OnePlus: «Батарея» → «Оптимизация батареи» → «Не оптимизировать». Там же: ⋮ → «Дополнительная оптимизация» → выключить «Глубокую оптимизацию» / «Адаптивную батарею» и «Оптимизацию режима ожидания». Закрепить в недавних. Обновления прошивки могут сбрасывать эти настройки. https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/ , https://dontkillmyapp.com/oneplus

### 3.5. Лаунчер по умолчанию
- «Настройки» → «Управление приложениями» → «Приложения по умолчанию» → «Главный экран» (Home app). https://www.techbone.net/oppo/smartphone/default-system-launcher (сторонний; рус. название не подтверждено)
- Со сторонним лаунчером на ColorOS 16 пользователи жалуются на задержку и пустой экран при жесте «домой»; с кнопками навигации проблемы нет. На ColorOS 17 это, возможно, исправлено. https://niagaralauncher.featurebase.app/p/persistent-redraw-delayblank-screen-when-swiping-home-coloros , https://9to5google.com/?p=737933 . **Совет: кнопочная навигация.**

### 3.6. 24/7
- realme: «Настройки» → «Батарея» → «Состояние батареи» (Battery health) → «Лимит заряда» (Charging Limit, 80 %). Телефон сам предлагает его, если долго стоит на зарядке. Есть также «Умная зарядка» (Wise charging: 80 % в рабочие часы, realme UI 5.0) и старая «Оптимизированная ночная зарядка». https://realme.com/in/support/kw/doc/2123111 , https://www.realme.com/in/support/kw/doc/2198669 . Рус. названия — **не подтверждены**.
- OPPO/ColorOS 14–15: аналог («Оптимизированная зарядка», лимит 80 %) — **не подтверждено** официальным источником.
- OnePlus 13 (бета Android 16) — настраиваемый лимит заряда. https://m.gsmarena.com/newscomm-67367.php . Для старых OnePlus есть «Оптимизированная зарядка» (Optimized Charging, ночная). https://www.gsmarena.com/oneplus_has_a_new_optimized_charging_feature-news-40910.php
- На realme C11 и подобных бюджетных моделях лимита, скорее всего, нет — **не подтверждено**. Остаётся внешнее реле или розетка (0.6).

---

## 4. Huawei / Honor (EMUI 5–12, HarmonyOS 2–4, MagicOS)

### 4.1. Режим разработчика и отладка по USB
- Huawei: «Настройки» → «О телефоне» → «Версия HarmonyOS» или «Версия ПО» (на EMUI — «Номер сборки») ×7. Затем «Настройки» → «Система и обновления» → «Для разработчиков» → «Отладка по USB». https://consumer.huawei.com/ru/support/content/ru-ru00407281/ , https://consumer.huawei.com/au/support/content/en-gb15594059
- Недоступно в детском режиме «Цифрового баланса», под вторым пользователем и в PrivateSpace — нужен владелец (Owner). https://consumer.huawei.com/ru/support/content/ru-ru00407281/
- Honor (MagicOS): Settings > System options > About Phone > Build Number ×7; «Для разработчиков» в System options. https://www.honor.com/uk/club/topicdetail/tips-tricks-what-to-do-when-you-are-unable-to-turn-on-usb-debugging/topicid-9027/ (пост 2020 года; на новых MagicOS путь может отличаться — не подтверждено)

### 4.2. Дополнительные переключатели для adb
- **«Разрешить отладку по ADB только при зарядке»** (Allow ADB debugging in charge only mode). Без него отладка не работает или выключается, когда USB в режиме «Только зарядка». Официально рекомендуется включать вместе с «Отладкой по USB» и выбирать режим USB «Передача файлов» или «Передача фото». https://consumer.huawei.com/ru/support/content/ru-ru00407281/ , https://consumer.huawei.com/en/support/content/en-us00407281
- Включение отладки иногда сбрасывает режим USB обратно в «Только зарядка» — тогда переключить его ещё раз. https://support.blancco.com/x/QYAlAg
- Отключить «Проверка приложений по USB» (на старых — «Мониторинг приложений, установленных…»). На некоторых старых моделях: «Выбор настройки USB» → «RNDIS (USB Ethernet)». https://yandex.ru/support/addappter/ru/work/device-control
- Для панели: в Ven4Docking всегда советовать включить «только при зарядке». Тогда после перезагрузки (USB по умолчанию в зарядке) adb остаётся доступен.

### 4.3. Беспроводная отладка
- Huawei официально: «Беспроводная отладка» в «Для разработчиков» **недоступна**; использовать кабель. https://consumer.huawei.com/ca/support/content/en-us15997564/
- На HarmonyOS основной инструмент — `hdc` (аналог adb), а не adb. https://cloud.tencent.com/developer/article/2423808
- Honor на MagicOS (Android 11+) — наличие беспроводной отладки **не подтверждено**.

### 4.4. Чтобы приложение не убивали
- EMUI 8–10 / HarmonyOS: «Батарея» → «Запуск приложений» → выключить «Управлять всем автоматически» → для приложения «Управлять вручную» и включить все три переключателя (автозапуск, косвенный запуск, работа в фоне). https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/ , https://dontkillmyapp.com/huawei
- EMUI 6+: «Диспетчер батареи» → «План электропитания» → «Производительность»; «Защищённые приложения» → добавить. В карточке приложения → «Батарея»: выключить «Энергосберегающая подсказка», включить «Продолжить работу после выключения экрана». «Расширенные» → «Игнорировать оптимизацию батареи» → «Разрешить». https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/
- Выключить «Умную оптимизацию» (Smart tune-up) в «Диспетчере телефона». https://dontkillmyapp.com/huawei
- **PowerGenie** (часть EMUI 9+) убивает приложения не из белого списка Huawei, добавить туда своё нельзя. dontkillmyapp предлагает `pm uninstall -k --user 0 com.huawei.powergenie` (и `com.huawei.android.hwaps`). https://dontkillmyapp.com/huawei . **Удаление системного компонента — только по явному согласию владельца; в автоматический сценарий не включать.**
- EMUI 4: пользовательского решения нет. https://dontkillmyapp.com/huawei

### 4.5. Лаунчер по умолчанию
- «Настройки» → «Приложения» → «Приложения по умолчанию» → «Рабочий экран» (Launcher). https://androidcentral.com/how-change-your-default-launcher-emui-9 , https://www.techbone.net/huawei/smartphone/standard-app-for-launcher . Рус. название не подтверждено.
- **Ловушка:** EMUI возвращает свой лаунчер, если сторонний упал или завершился «аномально», а также после его обновления. https://docs.smartlauncher.net/device-specific-issues/huawei-honor , https://androidcentral.com/how-change-your-default-launcher-emui-9 . Ven4Docking стоит периодически проверять, кто сейчас HOME (`resolve-activity`).
- На китайских прошивках EMUI 9 смена лаунчера была заблокирована (позже патч). https://www.digitec.ch/en/page/emui-9-from-china-the-way-around-the-launcher-lock-in-your-import-phone-9821
- 42Gears описывает отдельную процедуру для киоск-лаунчера на Huawei. https://knowledgebase.42gears.com/article/how-to-set-surelock-as-default-launcher-app-in-huawei-devices/ (детали не проверены)

### 4.6. 24/7
- «Настройки» → «Батарея» → «Состояние батареи» → «Пользовательский предел зарядки» (70/80/90/100 %), только HarmonyOS 4.3.1+. Несовместим с «Умной зарядкой». https://consumer.huawei.com/en/support/content/en-us15851192/
- «Умная зарядка» (Smart charge) — останавливает заряд на 80 % по привычкам пользователя. https://consumer.huawei.com/sg/support/content/en-gb00772869
- Рус. названия — не подтверждены.

---

## 5. vivo / iQOO (Funtouch OS, OriginOS)

### 5.1. Режим разработчика и отладка по USB
- Новые модели: «Настройки» → «Управление системой» → «О телефоне» → «Информация о версии» → «Версия ПО» ×7. Затем «Управление системой» → «Для разработчиков» → «Отладка по USB». Funtouch 4–9: «Дополнительные настройки» (More settings) → «Для разработчиков». https://www.coolmuster.com/enable-usb-debugging-on-vivo.html (сторонний; рус. названия не подтверждены)

### 5.2. Дополнительные переключатели для adb
- «Установка через USB» (Install via USB) в «Для разработчиков». По слабым источникам, может запросить пароль vivo-аккаунта; `adb install` может зависать или падать с `INSTALL_FAILED_USER_RESTRICTED` / `INSTALL_CANCELED_BY_USER`, если на экране телефона не подтвердить запрос. https://coldfusion-example.blogspot.com/2026/03/solving-flutter-debug-install-failures.html (блог низкого доверия) — **не подтверждено**.
- Пользователи iQOO жалуются, что команды adb (в т. ч. удаление системных приложений) блокируются. https://community.iqoo.com/in/thread/82352 , https://community.iqoo.com/in/thread/60304 . Причина — **не подтверждено**.
- Нужен ли vivo аналог «Отключить контроль разрешений» для `settings put`/`appops` — **не подтверждено**.
- **Совет для UI:** во время `adb install` попросить пользователя смотреть на экран телефона и подтвердить запрос.

### 5.3. Беспроводная отладка
- Android 11+ — общий механизм Android. https://developer.android.com/tools/adb . У vivo — **не подтверждено**.

### 5.4. Чтобы приложение не убивали
- Автозапуск: «Настройки» → «Ещё» → «Приложения» → «Автозапуск». В Funtouch 2.6 и ниже: «i Manager» → «Менеджер приложений» → «Менеджер автозапуска». https://dontkillmyapp.com/vivo
- Android 13: «Настройки» → «Батарея» → «Высокое энергопотребление в фоне» (или «Управление фоновым энергопотреблением») → разрешить. В карточке приложения → «Батарея» → «Не оптимизировано». https://dontkillmyapp.com/vivo (рус. названия не подтверждены)
- Закрепить в недавних (замок / «Lockdown»). https://dontkillmyapp.com/vivo

### 5.5. Лаунчер по умолчанию
- Ограничения vivo для сторонних лаунчеров — **не подтверждено**; работа `set-home-activity` — не проверена.

### 5.6. 24/7
- Встроенный лимит заряда у vivo — **не подтверждено**.

---

## 6. Motorola (почти «чистый» Android, My UX / Hello UI)

### 6.1–6.3. Отладка
- «Настройки» → «О телефоне» → «Номер сборки» ×7. «Для разработчиков» — в «Система» (как в «чистом» Android). https://developer.android.com/studio/debug/dev-options . Особых переключателей для adb не найдено. Беспроводная отладка — стандартная, Android 11+.

### 6.4. Чтобы приложение не убивали
- «Настройки» → «Приложения» → приложение → «Использование батареи приложением» → «Без ограничений» (Unrestricted). Выключить «Адаптивную батарею». Через поиск по настройкам найти «Управление фоновыми приложениями» (Manage background apps) → «Всегда разрешать». https://dontkillmyapp.com/motorola , https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/ (рус. названия не подтверждены)
- Пункт «Improve battery while inactive» из приложения Battery care может убивать приложения даже в выключенном состоянии. dontkillmyapp предлагает `pm uninstall -k --user 0 com.motorola.batterycare` — **только по согласию владельца**. https://dontkillmyapp.com/motorola

### 6.5. Лаунчер
- Ограничений Motorola не найдено. Действуют общие правила (0.5).

### 6.6. 24/7
- «Настройки» → «Батарея» → «Защита батареи» (Battery protection): «Умное расписание» или «Ручное» (держать 80 % всегда или по часам). На моделях с 4 ГБ ОЗУ пункт называется «Защита от перезаряда» (Overcharge protection) и умеет меньше.
- **Ловушка для 24/7:** если 7 дней подряд не было заряда до 100 %, защита сама отключается, чтобы телефон зарядился полностью. Для полного заряда из 80 % нужно отключить и снова подключить зарядку.
- Источник: https://help.motorola.com/hc/3767/16/global/en-us/T2101254315.html . Рус. названия не подтверждены.

---

## 7. Google Pixel и «чистый» Android (в т. ч. Android One)

### 7.1–7.3. Отладка
- «Настройки» → «О телефоне» → «Номер сборки» ×7. «Отладка по USB» — в «Система» → «Для разработчиков». Беспроводная отладка — с Android 11; на Android 16+ путь Settings > System > Developer options > Wireless debugging. https://developer.android.com/studio/debug/dev-options
- Дополнительных переключателей для adb нет.

### 7.4. Чтобы приложение не убивали
- Не включать для приложения «Ограничение работы в фоне» (Background restriction, Android 9+). Поставить «Не оптимизировать» / «Без ограничений». Крайняя мера — отключить Doze в «Для разработчиков» (влияет на весь телефон). https://dontkillmyapp.com/google
- Через adb закрывается полностью: команды из 0.4 работают.

### 7.5. Лаунчер
- «Настройки» → «Приложения» → «Приложения по умолчанию» → «Главный экран». https://support.google.com/pixelphone/answer/6271667?hl=en (рус. название пункта не подтверждено). `set-home-activity` и роль HOME работают по AOSP (0.3).

### 7.6. 24/7
- «Настройки» → «Батарея» → «Состояние батареи» (Battery health) → «Использовать оптимизацию зарядки» (Use charging optimization) → «Адаптивная зарядка» или «Ограничить до 80 %». Pixel 6a и новее. https://support.google.com/pixelphone/answer/6090612
- При лимите 80 % и постоянном питании включается обход батареи (bypass charging): телефон питается напрямую от зарядки. https://www.androidauthority.com/pixel-bypass-charging-3507373/
- **Для панели на Pixel это лучший вариант.** Рус. названия не подтверждены.

---

## 8. Nokia (HMD, Android One)

- Отладка — как в «чистом» Android (раздел 7). https://developer.android.com/studio/debug/dev-options
- **Убийца приложений Evenwell «Энергосбережение»** (`com.evenwell.powersaving.g3`): HMD отключила его на Android 9+ в августе 2019, но на части моделей остался DuraSpeed (MediaTek). На Nokia 1 (Go) — `com.evenwell.emm`. Решения:
  - временно: «Приложения» → «Показать системные» → «Энергосбережение» → «Принудительная остановка» (после перезапуска возвращается);
  - через adb: `pm disable-user com.evenwell.powersaving.g3` — **только по согласию**.
  - Nokia 3.1/5.1: DuraSpeed выключается настройкой `setting.duraspeed.enabled` ≠ 1, но после перезагрузки сбрасывается.
  https://dontkillmyapp.com/nokia , https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/
- Лаунчер: ограничений не найдено. Лимит заряда — **не подтверждено**.

---

## 9. Tecno / Infinix / itel (HiOS, XOS, itel OS — Transsion)

### 9.1. Отладка
- Tecno: «Настройки» → «Мой телефон» (на части моделей «О телефоне») → «Номер сборки» ×7. «Для разработчиков» — в «Система». https://forensics.socialtic.org/en/how-tos/02-how-to-enable-developer-options/ , https://www.coolmuster.com/enable-usb-debugging-on-tecno-infinix-itel.html (сторонние)
- На Android 13+ у Tecno/Infinix/itel в «Для разработчиков» может быть «Отключить контроль разрешений» — его советуют включить. https://www.mobikin.com/android-backup/enable-usb-debugging-on-tecno-infinix-itel.html (один сторонний источник — **не подтверждено**). Если `settings put`/`appops` падает с `SecurityException`, первым делом искать этот пункт.
- Беспроводная отладка — Android 11+, у Transsion не проверено.

### 9.2. Чтобы приложение не убивали
- Tecno:
  - «Настройки» → «Battery Lab» → «Battery Saving Settings» → выключить «Power Saving Management For Apps»;
  - «Phone Master» → «Toolbox» → «Auto-start management» → разрешить;
  - в «Phone Master» выключить «Power Boost»;
  - выключить «Screen off push block» и «Screen off sleep»;
  - закрепить в недавних.
  https://dontkillmyapp.com/tecno
- Infinix: приложение → «Батарея» → «Без ограничений»; «App management» → «Auto-start management» → включить; «Special App Access» → «Battery Optimisation» → «Not Optimised». https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/
- itel — отдельных данных нет, **не подтверждено** (вероятно, как у Tecno, т. к. Transsion).
- Рус. названия — не подтверждены (источники на англ.).

### 9.3–9.4. Лаунчер и 24/7
- Ограничения лаунчеров у Transsion — **не подтверждено**.
- Лимит заряда у части Infinix есть. https://m.gsmarena.com/newscomm-67585.php . Путь — **не подтверждено**.

---

## 10. Meizu (Flyme) — кратко

- Отладка: «Спец. возможности» → «Для разработчиков» → «Отладка по USB». «Безопасность» → «Разрешения для ПО» → «Установка ПО через USB» — выключить (по инструкции Яндекса). https://yandex.ru/support/addappter/ru/work/device-control
- Фон:
  - «Расширенные настройки» → «Менеджер батареи» → «Управление питанием» → «Производительность»;
  - «Защищённые приложения» → добавить;
  - приложение → «Батарея» → «Работа при выключенном экране»;
  - «Безопасность» → «Разрешения» → «Фоновые процессы».
  https://adguard.com/kb/ru/adguard-for-android/solving-problems/background-work/ , https://dontkillmyapp.com/meizu
- Остальное (беспроводная отладка, лаунчер, лимит заряда) — **не подтверждено**.

---

## 11. Сводная таблица «что включить, кроме Отладки по USB»

| Бренд | Обязательный доп. пункт | Ошибка без него | Уверенность |
|---|---|---|---|
| Xiaomi/Redmi/POCO | «Установка через USB» + «Отладка по USB (настройки безопасности)»; Mi-аккаунт; возможно, SIM | `INSTALL_FAILED_USER_RESTRICTED`; `Permission denial … WRITE_SECURE_SETTINGS` | Высокая (SIM — средняя) |
| Samsung One UI 6+ | Выключить «Автоблокировку» | «Отладка USB» серая, команды по USB блокируются | Высокая (форумы + офиц. описание блокировки USB-команд) |
| OPPO / realme / OnePlus | «Отключить контроль разрешений» (+ перезагрузка) | `SecurityException` на `settings`/`pm grant`/`appops` | Высокая (подтверждено на realme C11) |
| Huawei / Honor | «Разрешить отладку по ADB только при зарядке»; режим USB «Передача файлов» | Отладка выключается или недоступна | Высокая (офиц. Huawei) |
| vivo / iQOO | «Установка через USB»; подтверждение на экране; возможно, пароль vivo | Установка отменяется или зависает | Низкая — не подтверждено |
| Tecno/Infinix/itel | Возможно, «Отключить контроль разрешений» (Android 13+) | `SecurityException` | Низкая |
| Meizu | «Установка ПО через USB» (по Яндексу — выключить) | — | Средняя |
| Pixel, Motorola, Nokia | Ничего | — | Высокая |

---

## 12. Итог: что надёжно, а что нет

**Надёжно (официальная документация или несколько независимых источников):**
- Общий путь к «Для разработчиков» и отладке; беспроводная отладка только с Android 11; `adb tcpip` для ≤10.
- Минимальные версии команд: `cmd` (7.0), `set-home-activity` (7.0), `locksettings` (8.0), `RUN_ANY_IN_BACKGROUND` (9.0 — на 7–8 нужен `RUN_IN_BACKGROUND`), `set-standby-bucket` (9.0), роль HOME (10).
- Пороги `targetSdk` для установки: ≥23 на 14, ≥24 на 15.
- WebView на 7–9 заморожен (Chrome 119 / 138); сейчас Chrome и WebView требуют Android 10+.
- Xiaomi: оба USB-переключателя и Mi-аккаунт. ColorOS/realme/OnePlus: «Отключить контроль разрешений». Huawei: «только при зарядке» и отсутствие беспроводной отладки. Samsung: «Автоблокировка» блокирует USB-команды.
- Лимиты заряда Samsung (One UI 6.1+ / 7+), Pixel (6a+), Motorola, Huawei (HarmonyOS 4.3.1+), Xiaomi (новые модели), realme (новые модели) — включаются без root, но только в интерфейсе телефона.
- На зарядке штатные Doze и App Standby почти не действуют; угроза — OEM-«убийцы».

**Не подтверждено или противоречиво:**
- Xiaomi: обязательна ли SIM (только сторонние гайды); поведение HyperOS 2+ (есть сообщения о сбоях даже со всеми переключателями).
- vivo/iQOO: пароль аккаунта, блокировки adb, аналог «контроля разрешений».
- Tecno/Infinix/itel: «Отключить контроль разрешений», лимиты заряда, точные пути.
- Влияние «Автоблокировки» Samsung на беспроводную отладку; путь «Защиты 85 %» на старых One UI.
- Работа `cmd package set-home-activity` и роли HOME на MIUI/HyperOS, ColorOS, EMUI (по AOSP должна работать, но оболочки могут вмешиваться). Проверять результат каждый раз.
- adb-команды для включения встроенных лимитов заряда (Samsung, Pixel, Xiaomi) — не найдены; не использовать.
- Русские названия большинства пунктов вне Samsung, Huawei, Xiaomi (частично), OPPO/realme (developer) — переведены по смыслу. Перед показом в UI их надо сверить на живых устройствах.
- Действуют ли «ограниченные настройки» Android 13+ на приложения, установленные через `adb install`.
- Безопасность удаления OEM-компонентов (PowerGenie, Motorola batterycare, Evenwell) — только по dontkillmyapp. Делать только с явного согласия владельца.
