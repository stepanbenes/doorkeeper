// Modul mikrofonu s analogovým výstupem

// nastavení propojovacích pinů
#define NOISE_THRESHOLD_PIN 2
#define NOISE_LEVEL_PIN A0
#define BUTTON_PIN 3
#define LED_PIN 13
#define BUZZER_PIN 8
#define RX_PIN 11
#define TX_PIN 10

#define BUZZER_DURATION 2000

// připojení knihovny SoftwareSerial
#include <SoftwareSerial.h>
// inicializace Bluetooth modulu z knihovny SoftwareSerial
SoftwareSerial bluetooth(TX_PIN, RX_PIN);

volatile bool button_state;
volatile bool previous_button_state;

unsigned long buzzer_start_time;

int noise_level, maximum;
long casPreruseni;

void setup() {
  // inicializace komunikace po sériové lince
  // rychlostí 9600 baud
  Serial.begin(9600);

  // zahájení komunikace s Bluetooth modulem
  // skrze Softwarovou sériovou linku rychlostí 9600 baud
  bluetooth.begin(9600);
  
  // nastavení LED diody jako výstupní a její vypnutí
  pinMode(LED_PIN, OUTPUT);
  digitalWrite(LED_PIN, LOW);

  // nastaveni RELAY
  pinMode(BUZZER_PIN, OUTPUT);
  digitalWrite(BUZZER_PIN, HIGH); // LOW == relay is connected, HIGH == relay is disconnected
  buzzer_start_time = 0;
  
  // nastaveni BELL a BUTTON
  pinMode(NOISE_THRESHOLD_PIN, INPUT);
  pinMode(BUTTON_PIN, INPUT_PULLUP);

  attachInterrupt(digitalPinToInterrupt(NOISE_THRESHOLD_PIN), noise_interrupt, RISING);
  attachInterrupt(digitalPinToInterrupt(BUTTON_PIN), button_interrupt, CHANGE);

  button_state = previous_button_state = !digitalRead(BUTTON_PIN); // Pull-up resistors invert the logic, so true == off, false == on

  on_button_changed();
}

void loop() {

  // TODO: read NOISE_LEVEL_PIN
  //int noise_level = analogRead(NOISE_LEVEL_PIN);
  //Serial.println(noise_level);
 

  if (button_state != previous_button_state) {
    on_button_changed();
    previous_button_state = button_state;
  }

  if (digitalRead(BUZZER_PIN) == LOW && ((millis() - buzzer_start_time) >= (unsigned long)BUZZER_DURATION)) { // overflow should not matter if calculating in unsigned integer arithmetics
    digitalWrite(BUZZER_PIN, HIGH); // turn off buzzer
  }

  byte BluetoothData;
  // kontrola Bluetooth komunikace, pokud je dostupná nová
  // zpráva, tak nám tato funkce vrátí počet jejích znaků
  if (bluetooth.available() > 0) {
    // načtení prvního znaku ve frontě do proměnné
    BluetoothData=bluetooth.read();
    // dekódování přijatého znaku
    switch (BluetoothData) {
      // každý case obsahuje dekódování jednoho znaku
      case '0':
        // v případě přijetí znaku nuly vypneme LED diodu
        // a vypíšeme hlášku zpět do Bluetooth zařízení
        digitalWrite(LED_PIN, LOW);
        bluetooth.println("LED off");
        break;
      case '1':
        // v případě přijetí jedničky zapneme LED diodu
        // a vypíšeme hlášku zpět do Bluetooth zařízení
        digitalWrite(LED_PIN, HIGH);
        bluetooth.println("LED on");
        break;
      case 'a':
        // v případě přejetí znaku 'a' vypíšeme
        // čas od spuštění Arduina
        bluetooth.print("Uptime: ");
        bluetooth.print(millis());
        bluetooth.println(" ms");
        break;
      case 'b':
        // zde je ukázka načtení většího počtu informací,
        // po přijetí znaku 'b' tedy postupně tiskneme 
        // další znaky poslané ve zprávě
        bluetooth.print("Nacitam zpravu: ");
        BluetoothData=bluetooth.read();
        // v této smyčce zůstáváme do té doby,
        // dokud jsou nějaké znaky ve frontě
        while (bluetooth.available() > 0) {
          bluetooth.write(BluetoothData);
          Serial.write(BluetoothData);
          // krátká pauza mezi načítáním znaků
          delay(10);
          BluetoothData=bluetooth.read();
        }
        bluetooth.println();
        break;
      case 'x':
        //digitalWrite(BUZZER_PIN, digitalRead(BUZZER_PIN) ? LOW : HIGH); // toggle buzzer pin
        buzzer_start_time = millis(); // remember buzzer start time
        digitalWrite(BUZZER_PIN, LOW); // turn on buzzer
        break;
      case '\r':
        // přesun na začátek řádku - znak CR
        break;
      case '\n':
        // odřádkování - znak LF
        break;
      default:
        // v případě přijetí ostatních znaků
        // vytiskneme informaci o neznámé zprávě
        bluetooth.println("invalid command");
    }
  }

  delay(50);
}

void noise_interrupt() {
  Serial.println("noise!");
  bluetooth.println("noise!");
}

void button_interrupt() {
  button_state = !digitalRead(BUTTON_PIN);
}

void on_button_changed() {
  if (button_state) {
    Serial.println("button on");
    bluetooth.println("button on");
  }
  else {
    Serial.println("button off");
    bluetooth.println("button off");
  }
}
