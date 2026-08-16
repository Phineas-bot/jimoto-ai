import 'dart:async';
import 'dart:ui' show AppExitResponse;

import 'package:flutter/material.dart';
import 'package:gixgiz_desktop/app/app_identity.dart';
import 'package:gixgiz_desktop/app/app_routes.dart';
import 'package:gixgiz_desktop/features/chat/chat_page.dart';
import 'package:gixgiz_desktop/app/app_theme.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/features/about/about_screen.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_page.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';
import 'package:gixgiz_desktop/shared/app_shell.dart';

class GixGizApp extends StatefulWidget {
  const GixGizApp({required this.coreClient, super.key});

  final CoreClient coreClient;

  @override
  State<GixGizApp> createState() => _GixGizAppState();
}

class _GixGizAppState extends State<GixGizApp> {
  late final AppLifecycleListener _lifecycleListener;

  @override
  void initState() {
    super.initState();
    _lifecycleListener = AppLifecycleListener(
      onExitRequested: () async {
        await widget.coreClient.shutdown();
        return AppExitResponse.exit;
      },
    );
  }

  @override
  void dispose() {
    _lifecycleListener.dispose();
    unawaited(widget.coreClient.shutdown());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      onGenerateTitle: (context) => AppLocalizations.of(context).appTitle,
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: ThemeMode.system,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      initialRoute: AppRoutes.foundation,
      routes: {
        AppRoutes.foundation: (context) => AppShell(
          selectedIndex: 0,
          child: FoundationPage(coreClient: widget.coreClient),
        ),
        AppRoutes.chat: (context) => AppShell(
          selectedIndex: 1,
          child: ChatPage(coreClient: widget.coreClient),
        ),
        AppRoutes.about: (context) => const AppShell(
          selectedIndex: 2,
          child: AboutScreen(
            applicationId: AppIdentity.applicationId,
            releaseName: AppIdentity.releaseName,
          ),
        ),
      },
    );
  }
}
