/// Root widget and theme.
library;

import 'package:flutter/material.dart';

import 'ui/connect/connect_page.dart';

class MuxDeckApp extends StatelessWidget {
  const MuxDeckApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'MuxDeck',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        brightness: Brightness.dark,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF2D6CDF),
          brightness: Brightness.dark,
        ),
        scaffoldBackgroundColor: const Color(0xFF12141A),
      ),
      home: const ConnectPage(),
    );
  }
}
